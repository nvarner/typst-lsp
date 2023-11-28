//! Derived from https://github.com/typst/typst/blob/main/cli/src/main.rs

use core::fmt;
use std::fs::File;
use std::path::{Path, PathBuf};

use comemo::Prehashed;
use memmap2::Mmap;
use once_cell::sync::OnceCell;
use tracing::error;
use typst::eval::Bytes;
use typst::font::{Font, FontBook, FontInfo};
use walkdir::WalkDir;

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

    /// Include user-specified font paths.
    pub fn with_font_paths(mut self, font_paths: &[PathBuf]) -> Self {
        for font_path in font_paths {
            self.search_dir(font_path);
        }
        self
    }

    /// Search for fonts in the linux system font directories.
    #[cfg(all(unix, not(target_os = "macos")))]
    fn search_system(&mut self) {
        self.search_dir("/usr/share/fonts");
        self.search_dir("/usr/local/share/fonts");

        if let Some(dir) = dirs::font_dir() {
            self.search_dir(dir);
        }
    }

    /// Search for fonts in the macOS system font directories.
    #[cfg(target_os = "macos")]
    fn search_system(&mut self) {
        self.search_dir("/Library/Fonts");
        self.search_dir("/Network/Library/Fonts");
        self.search_dir("/System/Library/Fonts");

        if let Some(dir) = dirs::font_dir() {
            self.search_dir(dir);
        }
    }

    /// Search for fonts in the Windows system font directories.
    #[cfg(windows)]
    fn search_system(&mut self) {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());

        self.search_dir(Path::new(&windir).join("Fonts"));

        if let Some(roaming) = dirs::config_dir() {
            self.search_dir(roaming.join("Microsoft\\Windows\\Fonts"));
        }

        if let Some(local) = dirs::cache_dir() {
            self.search_dir(local.join("Microsoft\\Windows\\Fonts"));
        }
    }

    /// Search for all fonts in a directory recursively.
    fn search_dir(&mut self, path: impl AsRef<Path>) {
        for entry in WalkDir::new(path)
            .follow_links(true)
            .sort_by(|a, b| a.file_name().cmp(b.file_name()))
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("ttf" | "otf" | "TTF" | "OTF" | "ttc" | "otc" | "TTC" | "OTC"),
            ) {
                self.search_file(path);
            }
        }
    }

    /// Index the fonts in the file at the given path.
    fn search_file(&mut self, path: impl AsRef<Path>) {
        let path = path
            .as_ref()
            .canonicalize()
            .expect("could not canonicalize font file path");
        if let Ok(file) = File::open(&path) {
            if let Ok(mmap) = unsafe { Mmap::map(&file) } {
                for (i, info) in FontInfo::iter(&mmap).enumerate() {
                    self.book.push(info);
                    self.fonts.push(FontSlot {
                        path: Some(path.clone()),
                        index: i as u32,
                        font: OnceCell::new(),
                    });
                }
            }
        }
    }
}
