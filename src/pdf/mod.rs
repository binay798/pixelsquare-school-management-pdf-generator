//! Typst integration.
//!
//! Rendering happens in four steps, one per module:
//!
//! 1. [`templates`] resolves a template name to a file inside the templates
//!    directory and validates the name against path traversal.
//! 2. [`world`] implements [`typst::World`]: it feeds the template source, the
//!    JSON payload (through `sys.inputs`) and the fonts to the compiler.
//! 3. [`engine`] drives the compilation (`typst::compile`) and the PDF export
//!    (`typst_pdf::pdf`), limiting how many renders run at the same time.
//! 4. [`storage`] persists the produced bytes in the output directory.

pub mod engine;
pub mod fonts;
pub mod storage;
pub mod templates;
pub mod world;

pub use engine::{RenderedPdf, TypstEngine};
pub use fonts::FontCollection;
pub use storage::StoredPdf;
pub use templates::{TemplateRef, TemplateSet};
