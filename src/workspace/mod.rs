//! Holds types relating to the LSP concept of a "workspace". That is, the directories a user has
//! open in their editor, the files in them, the files they're currently editing, and so on.

use comemo::Prehashed;
use parking_lot::RwLock;
use typst::eval::Library;

use self::font_manager::FontManager;
use self::resource_manager::ResourceManager;
use self::source_manager::SourceManager;

pub mod font_manager;
pub mod resource;
pub mod resource_manager;
pub mod source;
pub mod source_manager;

pub struct Workspace {
    pub sources: SourceManager,
    pub resources: RwLock<ResourceManager>,

    // Needed so that `Workspace` can implement Typst's `World` trait
    pub typst_stdlib: Prehashed<Library>,
    pub fonts: FontManager,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            sources: Default::default(),
            resources: Default::default(),
            typst_stdlib: Prehashed::new(typst_library::build()),
            fonts: FontManager::builder().with_system().with_embedded().build(),
        }
    }
}
