//! Dynamic PDF generation microservice.
//!
//! The service accepts JSON payloads over HTTP, renders them with a
//! [Typst](https://typst.app) template from the `templates/` folder and stores
//! the resulting PDF in the `pdfs/` folder.
//!
//! Module overview:
//!
//! - [`api`] — axum router and HTTP handlers.
//! - [`config`] — environment driven configuration.
//! - [`error`] — error type shared by the whole service (maps to HTTP errors).
//! - [`models`] — typed request payloads (e.g. the invoice payload).
//! - [`pdf`] — Typst engine: fonts, templates, virtual world, storage.
//! - [`state`] — shared application state used by the handlers.

pub mod api;
pub mod config;
pub mod error;
pub mod models;
pub mod pdf;
pub mod state;
