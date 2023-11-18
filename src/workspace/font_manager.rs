//! Derived from https://github.com/typst/typst/blob/main/cli/src/main.rs

use core::fmt;
use std::path::{Path, PathBuf};

use comemo::Prehashed;
use fontdb::{Database, Source};
use once_cell::sync::OnceCell;
use tracing::error;
use typst::eval::Bytes;
use typst::font::{Font, FontBook, FontInfo};

use super::fs::local::LocalFs;
use super::fs::FsError;

/// Searches for fonts.
pub struct FontManager {
    book: Prehashed<FontBook>,
    fonts: Vec<FontSlot>,
}

impl FontManager {
    /// Create a new, empty font manager `Builder`.
    pub fn builder() -> Builder {
        Builder::new()
    }

    pub fn book(&self) -> &Prehashed<FontBook> {
        &self.book
    }

    pub fn font(&self, id: usize) -> Option<Font> {
        let slot = self.fonts.get(id)?;
        let font = slot.get_font().cloned();
        match font {
            Ok(font) => Some(font),
            Err(err) => {
                error!(%err, font_id = id, "failed to load font");
                None
            }
        }
    }

    pub fn clear(&mut self) {
        self.fonts.iter_mut().for_each(|font| font.invalidate());
    }
}

impl fmt::Debug for FontManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FontManager")
            .field("book", &"...")
            .field("fonts", &"...")
            .finish()
    }
}

// TODO: special handling for fonts that are in a project?

/// Holds details about the location of a font and lazily the font itself.
#[derive(Debug)]
struct FontSlot {
    /// If `None`, the font is embedded
    path: Option<PathBuf>,
    index: u32,
    font: OnceCell<Font>,
}

impl FontSlot {
    pub fn get_font(&self) -> FontResult<&Font> {
        self.font.get_or_try_init(|| self.init())
    }

    fn init(&self) -> FontResult<Font> {
        let path = self.path().expect("should not init font without path");
        let data = LocalFs::read_path_raw(path)?;

        Font::new(data.into(), self.index).ok_or(FontError::Parse)
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn invalidate(&mut self) {
        // don't invalidate embedded fonts
        if self.path.is_some() {
            self.font.take();
        }
    }
}

pub type FontResult<T> = Result<T, FontError>;

#[derive(thiserror::Error, Debug)]
pub enum FontError {
    #[error(transparent)]
    Fs(#[from] FsError),
    #[error("failed to parse font")]
    Parse,
}

pub struct Builder {
    book: FontBook,
    fonts: Vec<FontSlot>,
}

impl Builder {
    fn new() -> Self {
        Self {
            book: FontBook::new(),
            fonts: Vec::new(),
        }
    }

    /// Build into a `FontManager`.
    pub fn build(self) -> FontManager {
        FontManager {
            book: Prehashed::new(self.book),
            fonts: self.fonts,
        }
    }

    /// Add fonts that are embedded in the binary.
    pub fn with_embedded(mut self) -> Self {
        let mut add = |bytes: &'static [u8]| {
            let bytes = Bytes::from_static(bytes);
            for (i, font) in Font::iter(bytes).enumerate() {
                self.book.push(font.info().clone());
                self.fonts.push(FontSlot {
                    path: None,
                    index: i as u32,
                    font: OnceCell::with_value(font),
                });
            }
        };

        // Embed default fonts.
        add(include_bytes!("../../assets/fonts/LinLibertine_R.ttf"));
        add(include_bytes!("../../assets/fonts/LinLibertine_RB.ttf"));
        add(include_bytes!("../../assets/fonts/LinLibertine_RBI.ttf"));
        add(include_bytes!("../../assets/fonts/LinLibertine_RI.ttf"));
        add(include_bytes!("../../assets/fonts/NewCMMath-Book.otf"));
        add(include_bytes!("../../assets/fonts/NewCMMath-Regular.otf"));
        add(include_bytes!("../../assets/fonts/NewCM10-Regular.otf"));
        add(include_bytes!("../../assets/fonts/NewCM10-Bold.otf"));
        add(include_bytes!("../../assets/fonts/NewCM10-Italic.otf"));
        add(include_bytes!("../../assets/fonts/NewCM10-BoldItalic.otf"));
        add(include_bytes!("../../assets/fonts/DejaVuSansMono.ttf"));
        add(include_bytes!("../../assets/fonts/DejaVuSansMono-Bold.ttf"));

        self
    }

    /// Include system fonts.
    pub fn with_system(mut self) -> Self {
        self.search_system();
        self
    }

    /// Search for fonts in the system font directories.
    fn search_system(&mut self) {
        let mut db = Database::new();

        // System fonts have second priority.
        db.load_system_fonts();

        for face in db.faces() {
            let path = match &face.source {
                Source::File(path) | Source::SharedFile(path, _) => path,
                // We never add binary sources to the database, so there
                // shouldn't be any.
                Source::Binary(_) => continue,
            };

            let info = db
                .with_face_data(face.id, FontInfo::new)
                .expect("database must contain this font");

            if let Some(info) = info {
                self.book.push(info);
                self.fonts.push(FontSlot {
                    path: Some(path.clone()),
                    index: face.index,
                    font: OnceCell::new(),
                });
            }
        }
    }
}
