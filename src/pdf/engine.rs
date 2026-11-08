//! The Typst render engine.
//!
//! [`TypstEngine`] ties everything together: it resolves the template, builds
//! the [`PdfWorld`], compiles it with `typst::compile` and exports the result
//! with `typst_pdf::pdf`. Compilation is CPU bound, so it runs on a blocking
//! thread and the number of parallel renders is capped by a semaphore.

use std::path::Path;
use std::sync::Arc;

use tokio::sync::Semaphore;
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, pdf};

use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use crate::pdf::fonts::FontCollection;
use crate::pdf::storage::ensure_dir;
use crate::pdf::templates::{TemplateRef, TemplateSet};
use crate::pdf::world::{PdfWorld, format_diagnostic};

/// A rendered document, ready to be stored or streamed.
#[derive(Debug, Clone)]
pub struct RenderedPdf {
    /// The PDF file content.
    pub bytes: Vec<u8>,
    /// Amount of pages in the document.
    pub pages: usize,
}

/// Compiles Typst templates into PDFs.
pub struct TypstEngine {
    templates: TemplateSet,
    fonts: Arc<FontCollection>,
    permits: Semaphore,
}

impl TypstEngine {
    /// Builds the engine: loads the fonts and makes sure that the output
    /// directory exists.
    pub fn new(config: &AppConfig) -> AppResult<Self> {
        if !config.templates_dir.is_dir() {
            return Err(AppError::Storage(format!(
                "templates directory `{}` does not exist; nothing can be rendered",
                config.templates_dir.display()
            )));
        }

        ensure_dir(&config.output_dir)?;

        let fonts = Arc::new(FontCollection::load(config));
        tracing::info!(
            fonts = fonts.len(),
            system_fonts = config.load_system_fonts,
            "fonts loaded"
        );

        Ok(Self {
            templates: TemplateSet::new(config.templates_dir.clone()),
            fonts,
            permits: Semaphore::new(config.max_concurrent_renders),
        })
    }

    /// Builds an engine with the embedded fonts only. Used by the tests.
    pub fn with_embedded_fonts(config: &AppConfig) -> AppResult<Self> {
        ensure_dir(&config.output_dir)?;

        Ok(Self {
            templates: TemplateSet::new(config.templates_dir.clone()),
            fonts: Arc::new(FontCollection::embedded_only()),
            permits: Semaphore::new(config.max_concurrent_renders),
        })
    }

    /// The templates that can be rendered.
    pub fn templates(&self) -> &TemplateSet {
        &self.templates
    }

    /// Renders `template` with the given JSON payload.
    pub async fn render(&self, template: &str, data: &serde_json::Value) -> AppResult<RenderedPdf> {
        let template = self.templates.resolve(template)?;
        let payload = serde_json::to_string(data)
            .map_err(|err| AppError::Internal(format!("could not serialize the payload: {err}")))?;

        let fonts = Arc::clone(&self.fonts);
        let root = self.templates.root().to_path_buf();

        // Rendering is CPU bound, so it must not run on the async runtime.
        // The permit is held until the blocking task is done, which keeps the
        // amount of concurrent renders bounded.
        let _permit = self
            .permits
            .acquire()
            .await
            .map_err(|_| AppError::Internal("the render queue is closed".to_string()))?;

        tokio::task::spawn_blocking(move || render_blocking(&root, template, fonts, &payload))
            .await
            .map_err(|err| AppError::Internal(format!("rendering panicked: {err}")))?
    }
}

/// Compiles and exports a document. Runs on a blocking thread.
fn render_blocking(
    root: &Path,
    template: TemplateRef,
    fonts: Arc<FontCollection>,
    payload: &str,
) -> AppResult<RenderedPdf> {
    let world = PdfWorld::new(root.to_path_buf(), &template, fonts, payload);

    let document = compile(&world)?;
    let pages = document.pages().len();

    let bytes =
        pdf(&document, &PdfOptions::default()).map_err(|diagnostics| AppError::PdfExport {
            details: format_diagnostics(&world, &diagnostics),
        })?;

    Ok(RenderedPdf { bytes, pages })
}

/// Runs the Typst compiler and maps its diagnostics to [`AppError`].
fn compile(world: &PdfWorld) -> AppResult<PagedDocument> {
    let result = typst::compile::<PagedDocument>(world);

    for warning in &result.warnings {
        tracing::debug!(diagnostic = %format_diagnostic(world, warning), "typst warning");
    }

    result.output.map_err(|diagnostics| AppError::Template {
        details: format_diagnostics(world, &diagnostics),
    })
}

/// Joins the diagnostics of a failed compilation into one message per problem.
fn format_diagnostics(
    world: &PdfWorld,
    diagnostics: &[typst::diag::SourceDiagnostic],
) -> Vec<String> {
    diagnostics
        .iter()
        .take(MAX_REPORTED_DIAGNOSTICS)
        .map(|diagnostic| format_diagnostic(world, diagnostic))
        .chain((diagnostics.len() > MAX_REPORTED_DIAGNOSTICS).then(|| {
            format!(
                "… and {} more problems",
                diagnostics.len() - MAX_REPORTED_DIAGNOSTICS
            )
        }))
        .collect()
}

/// How many Typst diagnostics are reported back to the client.
const MAX_REPORTED_DIAGNOSTICS: usize = 5;
