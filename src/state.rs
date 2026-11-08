//! Shared application state.
//!
//! The state is created once at startup and shared by all handlers through an
//! `Arc`. It owns the configuration and the Typst engine (fonts, templates and
//! the render semaphore), which are both expensive to build and therefore must
//! not be recreated per request.

use std::sync::Arc;

use crate::config::AppConfig;
use crate::error::AppResult;
use crate::pdf::TypstEngine;

/// Everything a handler needs.
pub struct AppState {
    config: Arc<AppConfig>,
    engine: Arc<TypstEngine>,
}

impl AppState {
    /// Builds the state: loads the fonts and validates the directories.
    pub fn new(config: AppConfig) -> AppResult<Arc<Self>> {
        let engine = TypstEngine::new(&config)?;
        Ok(Arc::new(Self {
            config: Arc::new(config),
            engine: Arc::new(engine),
        }))
    }

    /// Builds the state with the fonts embedded in the binary only.
    ///
    /// Skips the (slow) system font scan and is therefore used by the tests.
    pub fn with_embedded_fonts(config: AppConfig) -> AppResult<Arc<Self>> {
        let engine = TypstEngine::with_embedded_fonts(&config)?;
        Ok(Arc::new(Self {
            config: Arc::new(config),
            engine: Arc::new(engine),
        }))
    }

    /// The configuration the service was started with.
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// The render engine.
    pub fn engine(&self) -> &TypstEngine {
        &self.engine
    }

    /// The directory generated PDFs are written to.
    pub fn output_dir(&self) -> &std::path::Path {
        &self.config.output_dir
    }
}
