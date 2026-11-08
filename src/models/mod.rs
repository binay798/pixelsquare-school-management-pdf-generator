//! Typed request payloads.
//!
//! Every supported document type owns a module here. The modules are
//! responsible for parsing, validating and turning the payload into the JSON
//! view model that the matching Typst template consumes.

pub mod invoice;
pub mod report_card;

pub(crate) mod format;

pub use invoice::{InvoiceItem, InvoicePayload, Party};
pub use report_card::{Attendance, Guardian, ReportCardPayload, ReportCardSubject, School};
