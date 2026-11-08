//! Implementation of [`typst::World`].
//!
//! The world is the bridge between the service and the Typst compiler. It is
//! created per render request and provides:
//!
//! - the **main source**: the template that was requested,
//! - the **payload**: the JSON view model, reachable from the template as
//!   `sys.inputs.at("data")`,
//! - **fonts**: the shared [`FontCollection`],
//! - **files**: every other file of the templates directory (imports, images,
//!   data files) is read from disk on demand,
//! - **today**: the current date, so `datetime.today()` behaves like a normal
//!   Typst run.

use chrono::{Datelike, TimeDelta, Timelike};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Dict, Duration, Value};
use typst::syntax::{FileId, Source, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, WorldExt};

use crate::pdf::fonts::FontCollection;
use crate::pdf::templates::TemplateRef;

/// Key under which the JSON payload is exposed in `sys.inputs`.
pub const INPUT_DATA: &str = "data";
/// Key under which the template name is exposed in `sys.inputs`.
pub const INPUT_TEMPLATE: &str = "template";

/// Typst world backed by the templates directory and a JSON payload.
pub struct PdfWorld {
    library: LazyHash<Library>,
    main: FileId,
    fonts: Arc<FontCollection>,
    root: PathBuf,
    sources: Mutex<HashMap<FileId, Source>>,
    now: Datetime,
}

impl PdfWorld {
    /// Creates a world for the given template and payload.
    ///
    /// `payload` is the JSON view model; it is exposed through `sys.inputs`
    /// where templates read it with `json.decode(sys.inputs.at("data"))`.
    pub fn new(
        root: PathBuf,
        template: &TemplateRef,
        fonts: Arc<FontCollection>,
        payload: &str,
    ) -> Self {
        let mut inputs = Dict::new();
        inputs.insert(INPUT_DATA.into(), Value::Str(payload.into()));
        inputs.insert(
            INPUT_TEMPLATE.into(),
            Value::Str(template.name().to_string().into()),
        );

        let library = Library::builder().with_inputs(inputs).build();

        Self {
            library: LazyHash::new(library),
            main: template.file_id(),
            fonts,
            root,
            sources: Mutex::new(HashMap::new()),
            now: now(),
        }
    }

    /// Maps a virtual path onto the real file inside the templates directory.
    fn real_path(&self, id: FileId) -> FileResult<PathBuf> {
        // Packages are not supported: templates may only use local files.
        if !matches!(id.root(), VirtualRoot::Project) {
            return Err(FileError::NotFound(PathBuf::from(
                id.vpath().get_without_slash(),
            )));
        }

        let path = self.root.join(id.vpath().get_without_slash());

        // A virtual path is normalised, so this can only fail if the caller
        // constructed a `FileId` by hand. Guard against it anyway.
        if !path.starts_with(&self.root) {
            return Err(FileError::AccessDenied);
        }

        Ok(path)
    }

    /// Reads and parses a file of the templates directory.
    fn load_source(&self, id: FileId) -> FileResult<Source> {
        let path = self.real_path(id)?;
        let text = fs::read_to_string(&path).map_err(|err| FileError::from_io(err, &path))?;
        Ok(Source::new(id, text))
    }
}

impl World for PdfWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if let Ok(sources) = self.sources.lock()
            && let Some(source) = sources.get(&id)
        {
            return Ok(source.clone());
        }

        let source = self.load_source(id)?;

        if let Ok(mut sources) = self.sources.lock() {
            sources.insert(id, source.clone());
        }

        Ok(source)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if let Ok(source) = self.source(id) {
            return Ok(Bytes::from_string(source.text().to_string()));
        }

        let path = self.real_path(id)?;
        fs::read(&path)
            .map(Bytes::new)
            .map_err(|err| FileError::from_io(err, &path))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        let Some(offset) = offset else {
            // Without an offset the world reports the date it was created for,
            // so a single render always sees a stable "today".
            return Some(self.now);
        };

        // `datetime.today(offset: n)` shifts the current date by `n` hours.
        let [weeks, days, hours, _, _] = offset.decompose();
        let shift = TimeDelta::try_hours(weeks * 168 + days * 24 + hours)?;
        let now = chrono::Local::now() + shift;

        Datetime::from_ymd_hms(
            now.year(),
            now.month() as u8,
            now.day() as u8,
            now.hour() as u8,
            now.minute() as u8,
            now.second() as u8,
        )
    }
}

/// The current local date and time as a Typst [`Datetime`].
fn now() -> Datetime {
    let now = chrono::Local::now();
    Datetime::from_ymd_hms(
        now.year(),
        now.month() as u8,
        now.day() as u8,
        now.hour() as u8,
        now.minute() as u8,
        now.second() as u8,
    )
    .unwrap_or_else(|| Datetime::from_ymd(1970, 1, 1).expect("1970-01-01 is a valid date"))
}

/// Formats a Typst diagnostic as `message (file:line:column)`.
pub fn format_diagnostic(world: &PdfWorld, diagnostic: &typst::diag::SourceDiagnostic) -> String {
    let message = diagnostic.message.to_string();

    let Some(id) = diagnostic.span.id() else {
        return message;
    };

    let Some(range) = world.range(diagnostic.span) else {
        return message;
    };

    let Ok(source) = world.source(id) else {
        return message;
    };

    match source.lines().byte_to_line_column(range.start) {
        Some((line, column)) => format!(
            "{message} ({}:{}:{})",
            id.vpath().get_without_slash(),
            line + 1,
            column + 1
        ),
        None => message,
    }
}
