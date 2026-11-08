//! Resolution of template names to files inside the templates directory.
//!
//! Templates are addressed by name, e.g. `invoice` resolves to
//! `<templates_dir>/invoice.typ`. Names are validated before they touch the file
//! system so a request can never escape the templates directory.
//!
//! Inside a template every file of the templates directory can be imported
//! through a relative path (e.g. `#import "partials/header.typ": header`); the
//! [`crate::pdf::world::PdfWorld`] serves those files from the directory.

use std::fs;
use std::path::{Path, PathBuf};

use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};

use crate::error::{AppError, AppResult};

/// Extension of Typst source files.
const TEMPLATE_EXTENSION: &str = "typ";

/// A resolved template: its name, its virtual path and its file on disk.
#[derive(Debug, Clone)]
pub struct TemplateRef {
    name: String,
    vpath: VirtualPath,
    path: PathBuf,
}

impl TemplateRef {
    /// Name the template was requested with, e.g. `invoice`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Virtual path of the template inside the Typst project, e.g.
    /// `invoice.typ`.
    pub fn vpath(&self) -> &VirtualPath {
        &self.vpath
    }

    /// Real path of the template on disk.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The interned Typst file id of the template.
    pub fn file_id(&self) -> FileId {
        RootedPath::new(VirtualRoot::Project, self.vpath.clone()).intern()
    }
}

/// The set of templates that is served by the application.
#[derive(Debug, Clone)]
pub struct TemplateSet {
    root: PathBuf,
}

impl TemplateSet {
    /// Creates a set for the given templates directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The directory the templates are read from.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a template name to its file on disk.
    ///
    /// Fails with [`AppError::InvalidTemplateName`] when the name is not a safe
    /// file stem and with [`AppError::TemplateNotFound`] when the file is
    /// missing.
    pub fn resolve(&self, name: &str) -> AppResult<TemplateRef> {
        let name = name.trim();

        if !is_valid_name(name) {
            return Err(AppError::InvalidTemplateName(name.to_string()));
        }

        let file_name = format!("{name}.{TEMPLATE_EXTENSION}");
        let path = self.root.join(&file_name);

        if !path.is_file() {
            return Err(AppError::TemplateNotFound(name.to_string()));
        }

        let vpath = VirtualPath::new(&file_name)
            .map_err(|err| AppError::InvalidTemplateName(format!("{name}: {err}")))?;

        Ok(TemplateRef {
            name: name.to_string(),
            vpath,
            path,
        })
    }

    /// Names of all templates in the directory, sorted alphabetically.
    pub fn list(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };

        let mut names: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| {
                let path = entry.path();
                (path.extension()? == TEMPLATE_EXTENSION)
                    .then(|| path.file_stem()?.to_str().map(str::to_string))
                    .flatten()
            })
            .collect();

        names.sort();
        names
    }
}

/// A template name may only contain ASCII letters, digits, `-` and `_`.
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crate_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn resolves_the_invoice_template() {
        let set = TemplateSet::new(crate_root().join("templates"));
        let template = set.resolve("invoice").expect("invoice template exists");
        assert_eq!(template.name(), "invoice");
        assert_eq!(template.vpath().get_without_slash(), "invoice.typ");
        assert!(template.path().is_file());
        assert!(set.list().contains(&"invoice".to_string()));
    }

    #[test]
    fn rejects_unsafe_names() {
        let set = TemplateSet::new(crate_root().join("templates"));

        for name in ["../Cargo", "partials/invoice", "invoice.typ", "", "  "] {
            let error = set.resolve(name).expect_err("name must be rejected");
            assert!(
                matches!(
                    error,
                    AppError::InvalidTemplateName(_) | AppError::TemplateNotFound(_)
                ),
                "unexpected error for `{name}`: {error}"
            );
        }
    }

    #[test]
    fn reports_missing_templates() {
        let set = TemplateSet::new(crate_root().join("templates"));
        let error = set
            .resolve("does-not-exist")
            .expect_err("template is missing");
        assert!(matches!(error, AppError::TemplateNotFound(_)));
    }
}
