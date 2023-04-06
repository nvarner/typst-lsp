//! Derived from https://github.com/typst/typst/blob/main/cli/src/main.rs

use std::fs::File;
use std::path::Path;

use anyhow::Context;
use comemo::Prehashed;
use memmap2::Mmap;
use once_cell::sync::OnceCell;
use tower_lsp::lsp_types::Url;
use typst::font::{Font, FontBook, FontInfo};
use typst::util::Buffer;
use walkdir::WalkDir;

use super::resource_manager::ResourceManager;

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

    pub fn font(&self, id: usize, resource_manager: &ResourceManager) -> Option<Font> {
        let slot = self.fonts.get(id)?;
        slot.get_font(resource_manager).as_ref().cloned().ok()
    }
}

/// Holds details about the location of a font and lazily the font itself.
struct FontSlot {
    /// If `None`, the font is embedded
    uri: Option<Url>,
    index: u32,
    font: OnceCell<anyhow::Result<Font>>,
}

impl FontSlot {
    pub fn get_font(&self, resource_manager: &ResourceManager) -> &anyhow::Result<Font> {
        self.font.get_or_init(|| self.init(resource_manager))
    }

    fn init(&self, resource_manager: &ResourceManager) -> anyhow::Result<Font> {
        let uri = self.uri.as_ref().context("could not get font url")?;
        let data = resource_manager
            .get_resource_by_uri(uri)
            .context("could not load font")?;
        Font::new(data.into(), self.index).context("could not parse font")
    }
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
            let buffer = Buffer::from_static(bytes);
            for (i, font) in Font::iter(buffer).enumerate() {
                self.book.push(font.info().clone());
                self.fonts.push(FontSlot {
                    uri: None,
                    index: i as u32,
                    font: OnceCell::from(Ok(font)),
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
        add(include_bytes!("../../assets/fonts/DejaVuSansMono.ttf"));
        add(include_bytes!("../../assets/fonts/DejaVuSansMono-Bold.ttf"));

        self
    }

    /// Include system fonts.
    pub fn with_system(mut self) -> Self {
        self.search_system();
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
                        uri: Some(Url::from_file_path(&path).unwrap()),
                        index: i as u32,
                        font: OnceCell::new(),
                    });
                }
            }
        }
    }
}
