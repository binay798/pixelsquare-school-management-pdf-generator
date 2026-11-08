//! Persistence of the generated PDFs.
//!
//! Every render is written to `pdfs/<uuid>.pdf`. The file name is generated
//! here, never taken from the request, so a payload cannot influence where a
//! file is written. Files are written to a temporary name first and then
//! renamed, so a client never sees a half written PDF.

use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::pdf::engine::RenderedPdf;

/// Extension of the generated documents.
pub const PDF_EXTENSION: &str = "pdf";

/// A PDF that was written to the output directory.
#[derive(Debug, Clone)]
pub struct StoredPdf {
    /// File name inside the output directory, e.g. `5f0c….pdf`.
    pub file_name: String,
    /// Absolute or configured-relative path of the file.
    pub path: PathBuf,
    /// Size of the file in bytes.
    pub size_bytes: usize,
    /// Amount of pages in the document.
    pub pages: usize,
}

/// Creates the output directory if it does not exist yet.
pub fn ensure_dir(dir: &Path) -> AppResult<()> {
    fs::create_dir_all(dir).map_err(|err| {
        AppError::Storage(format!(
            "could not create the output directory `{}`: {err}",
            dir.display()
        ))
    })
}

/// Writes a rendered document into `dir` and returns a handle to it.
pub fn save(dir: &Path, rendered: &RenderedPdf) -> AppResult<StoredPdf> {
    ensure_dir(dir)?;

    let file_name = format!("{}.{PDF_EXTENSION}", Uuid::new_v4());
    let path = dir.join(&file_name);
    let partial = dir.join(format!("{file_name}.partial"));

    fs::write(&partial, &rendered.bytes).map_err(|err| {
        AppError::Storage(format!("could not write `{}`: {err}", partial.display()))
    })?;

    fs::rename(&partial, &path).map_err(|err| {
        // Do not leave the temporary file behind when the rename fails.
        let _ = fs::remove_file(&partial);
        AppError::Storage(format!("could not finalize `{}`: {err}", path.display()))
    })?;

    Ok(StoredPdf {
        file_name,
        path,
        size_bytes: rendered.bytes.len(),
        pages: rendered.pages,
    })
}

/// Reads a previously stored PDF back from `dir`.
///
/// Fails with [`AppError::InvalidPayload`] when `file_name` is not a plain
/// `*.pdf` file name, which keeps the endpoint free of path traversal.
pub fn load(dir: &Path, file_name: &str) -> AppResult<Vec<u8>> {
    if !is_safe_file_name(file_name) {
        return Err(AppError::InvalidPayload(format!(
            "`{file_name}` is not a valid document name"
        )));
    }

    let path = dir.join(file_name);

    fs::read(&path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            AppError::DocumentNotFound(file_name.to_string())
        } else {
            AppError::Storage(format!("could not read `{}`: {err}", path.display()))
        }
    })
}

/// Whether `file_name` is a plain PDF file name without any path components.
pub fn is_safe_file_name(file_name: &str) -> bool {
    !file_name.is_empty()
        && file_name.len() <= 128
        && !file_name.contains(['/', '\\', ':'])
        && !file_name.starts_with('.')
        && file_name.ends_with(&format!(".{PDF_EXTENSION}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pdf-gen-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("temp dir can be created");
        dir
    }

    fn rendered(bytes: &[u8]) -> RenderedPdf {
        RenderedPdf {
            bytes: bytes.to_vec(),
            pages: 1,
        }
    }

    #[test]
    fn saves_and_loads_a_document() {
        let dir = temp_dir();

        let stored = save(&dir, &rendered(b"%PDF-1.7 hello")).expect("document is written");
        assert!(stored.file_name.ends_with(".pdf"));
        assert_eq!(stored.size_bytes as usize, b"%PDF-1.7 hello".len());
        assert!(stored.path.is_file());

        // No temporary files are left behind.
        let entries: Vec<_> = fs::read_dir(&dir)
            .expect("dir is readable")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(entries, vec![stored.file_name.clone()]);

        let loaded = load(&dir, &stored.file_name).expect("document is readable");
        assert_eq!(loaded, b"%PDF-1.7 hello");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_unsafe_file_names() {
        let dir = temp_dir();

        for name in ["../secret.pdf", "a/b.pdf", ".hidden.pdf", "report.txt", ""] {
            let error = load(&dir, name).expect_err("name must be rejected");
            assert!(matches!(error, AppError::InvalidPayload(_)), "{name}");
        }

        assert!(is_safe_file_name(
            "5f0c9b3e-6a0f-4f0c-9b6a-0f4f0c9b6a0f.pdf"
        ));
        assert!(!is_safe_file_name("invoice.PDF"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_documents_are_not_found_errors() {
        let dir = temp_dir();
        let error = load(&dir, "00000000-0000-0000-0000-000000000000.pdf")
            .expect_err("document does not exist");
        assert!(matches!(error, AppError::DocumentNotFound(_)));
        fs::remove_dir_all(&dir).ok();
    }
}
