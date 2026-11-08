//! Errors of the service and their HTTP representation.
//!
//! Every handler returns [`AppError`]; the `IntoResponse` implementation turns
//! it into a stable JSON error envelope:
//!
//! ```json
//! {
//!   "success": false,
//!   "error": { "code": "VALIDATION_FAILED", "message": "...", "details": ["..."] }
//! }
//! ```

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Result alias used across the service.
pub type AppResult<T> = Result<T, AppError>;

/// All errors that can occur while serving a request.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The requested template does not exist.
    #[error("template `{0}` was not found")]
    TemplateNotFound(String),

    /// The requested PDF does not exist in the output directory.
    #[error("document `{0}` was not found")]
    DocumentNotFound(String),

    /// The requested template name is not allowed (e.g. path traversal).
    #[error("invalid template name: {0}")]
    InvalidTemplateName(String),

    /// The request body could not be parsed into the expected payload.
    #[error("invalid payload: {0}")]
    InvalidPayload(String),

    /// The request body was larger than the configured limit.
    #[error("request body exceeds the maximum allowed size")]
    PayloadTooLarge,

    /// The payload was parsed but does not satisfy the business rules.
    #[error("payload validation failed")]
    Validation {
        /// One message per validation problem.
        details: Vec<String>,
    },

    /// The Typst template failed to compile or produced errors.
    #[error("template rendering failed")]
    Template {
        /// One message per Typst diagnostic.
        details: Vec<String>,
    },

    /// The document could not be exported to PDF.
    #[error("pdf export failed")]
    PdfExport {
        /// One message per Typst diagnostic.
        details: Vec<String>,
    },

    /// Reading or writing files failed.
    #[error("storage error: {0}")]
    Storage(String),

    /// Anything unexpected.
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    /// Builds a validation error from a list of problems.
    pub fn validation(details: Vec<String>) -> Self {
        Self::Validation { details }
    }

    /// Builds a template error from a list of Typst diagnostics.
    pub fn template(details: Vec<String>) -> Self {
        Self::Template { details }
    }

    /// The HTTP status code that matches this error.
    pub fn status(&self) -> StatusCode {
        match self {
            Self::TemplateNotFound(_) => StatusCode::NOT_FOUND,
            Self::DocumentNotFound(_) => StatusCode::NOT_FOUND,
            Self::InvalidTemplateName(_) | Self::InvalidPayload(_) => StatusCode::BAD_REQUEST,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Validation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Template { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::PdfExport { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Storage(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Stable machine readable error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::TemplateNotFound(_) => "TEMPLATE_NOT_FOUND",
            Self::DocumentNotFound(_) => "DOCUMENT_NOT_FOUND",
            Self::InvalidTemplateName(_) => "INVALID_TEMPLATE_NAME",
            Self::InvalidPayload(_) => "INVALID_PAYLOAD",
            Self::PayloadTooLarge => "PAYLOAD_TOO_LARGE",
            Self::Validation { .. } => "VALIDATION_FAILED",
            Self::Template { .. } => "TEMPLATE_ERROR",
            Self::PdfExport { .. } => "PDF_EXPORT_ERROR",
            Self::Storage(_) => "STORAGE_ERROR",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    /// Additional details that are sent to the client (if any).
    pub fn details(&self) -> Vec<String> {
        match self {
            Self::Validation { details }
            | Self::Template { details }
            | Self::PdfExport { details } => details.clone(),
            _ => Vec::new(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let details = self.details();

        if status.is_server_error() {
            tracing::error!(code = self.code(), error = %self, ?details, "request failed");
        } else {
            tracing::debug!(code = self.code(), error = %self, ?details, "request rejected");
        }

        let mut error = json!({
            "code": self.code(),
            "message": self.to_string(),
        });

        if !details.is_empty() {
            error["details"] = json!(details);
        }

        (status, Json(json!({ "success": false, "error": error }))).into_response()
    }
}
