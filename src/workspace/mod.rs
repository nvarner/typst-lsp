//! Holds types relating to the LSP concept of a "workspace". That is, the directories a user has
//! open in their editor, the files in them, the files they're currently editing, and so on.

use std::path::PathBuf;

use comemo::Prehashed;
use tower_lsp::lsp_types::Url;
use typst::diag::FileResult;
use typst::eval::Library;
use typst::file::FileId;

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
    pub fn source_manager(&self) -> &impl SourceManager {
        &self.files
    }

    pub fn source_manager_mut(&mut self) -> &mut impl SourceManager {
        &mut self.files
    }

    pub fn resource_manager(&self) -> impl ResourceManager + '_ {
        &self.files
    }

    pub fn font_manager(&self) -> &FontManager {
        &self.fonts
    }

    pub fn id_for(&self, uri: &Url) -> anyhow::Result<FileId> {
        self.files.uri_to_id(uri)
    }

    pub fn id_to_path(&self, id: FileId) -> FileResult<PathBuf> {
        self.files.id_to_path(id)
    }

    pub fn invalidate(&mut self, id: FileId) {
        self.files.file_mut(id).invalidate()
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
