//! Font discovery.
//!
//! Typst needs at least one font to render text. [`FontCollection`] combines
//! three sources, in this order:
//!
//! 1. the fonts embedded in the binary via `typst-assets` (Libertinus Serif,
//!    New Computer Modern, DejaVu Sans Mono) so a render never fails on a bare
//!    host,
//! 2. the fonts installed on the operating system (can be disabled with
//!    `PDF_GEN_DISABLE_SYSTEM_FONTS`),
//! 3. additional directories configured through `PDF_GEN_FONT_DIRS`.
//!
//! Fonts are loaded lazily: only the metadata is collected up front, the actual
//! font data is read from disk the first time the compiler asks for it.

use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst_kit::fonts::{self, FontStore};

use crate::config::AppConfig;

/// All fonts that are available to the compiler.
///
/// The struct owns the [`FontStore`] and therefore the font metadata that the
/// [`typst::World`] implementation hands to the compiler. Font data is loaded
/// lazily, the first time the compiler asks for a font.
pub struct FontCollection {
    store: FontStore,
}

impl FontCollection {
    /// Loads the embedded fonts and, depending on the configuration, the fonts
    /// installed on the host.
    ///
    /// Scanning the system fonts can take a moment; it happens once at startup.
    pub fn load(config: &AppConfig) -> Self {
        let mut store = FontStore::new();

        // Always available: fonts compiled into the binary.
        store.extend(fonts::embedded());

        if config.load_system_fonts {
            store.extend(fonts::system());
        }

        for dir in &config.extra_font_dirs {
            if dir.is_dir() {
                store.extend(fonts::scan(dir));
            } else {
                tracing::warn!(
                    directory = %dir.display(),
                    "extra font directory does not exist and is ignored"
                );
            }
        }

        Self { store }
    }

    /// Builds a collection from the fonts that ships with `typst-assets` only.
    ///
    /// This is considerably faster than [`FontCollection::load`] and is used by
    /// the tests.
    pub fn embedded_only() -> Self {
        let mut store = FontStore::new();
        store.extend(fonts::embedded());
        Self { store }
    }

    /// Metadata of all loaded fonts.
    pub fn book(&self) -> &LazyHash<FontBook> {
        self.store.book()
    }

    /// The font with the given index in the font book.
    pub fn font(&self, index: usize) -> Option<Font> {
        self.store.font(index)
    }

    /// Number of fonts that are known to the compiler.
    pub fn len(&self) -> usize {
        // Every font belongs to exactly one family, so the sum of all family
        // members is the amount of fonts.
        self.store
            .book()
            .families()
            .map(|(_, ids)| ids.count())
            .sum()
    }

    /// Whether no font at all could be loaded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_fonts_are_available() {
        let fonts = FontCollection::embedded_only();
        assert!(!fonts.is_empty(), "expected the embedded fonts to load");
        assert!(fonts.font(0).is_some(), "expected the first font to load");
    }
}
