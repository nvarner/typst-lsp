//! Holds types relating to the LSP concept of a "workspace". That is, the directories a user has
//! open in their editor, the files in them, the files they're currently editing, and so on.

use comemo::Prehashed;
use typst::eval::Library;

use self::file_manager::FileManager;
use self::font_manager::FontManager;
use self::resource_manager::ResourceManager;
use self::source_manager::SourceManager;

pub mod file_manager;
pub mod font_manager;
pub mod resource_manager;
pub mod source_manager;

pub struct Workspace {
    files: FileManager,
    fonts: FontManager,

    // Needed so that `Workspace` can implement Typst's `World` trait
    pub typst_stdlib: Prehashed<Library>,
}

impl Workspace {
    pub fn sources(&self) -> impl SourceManager + '_ {
        &self.files
    }

    pub fn resources(&self) -> impl ResourceManager + '_ {
        &self.files
    }

    pub fn fonts(&self) -> &FontManager {
        &self.fonts
    }

    pub fn clear(&mut self) {
        self.fonts.clear();
        self.files.clear();
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            files: FileManager::default(),
            typst_stdlib: Prehashed::new(typst_library::build()),
            fonts: FontManager::builder().with_system().with_embedded().build(),
        }
    }
}
