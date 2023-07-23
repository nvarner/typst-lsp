//! Holds types relating to the LSP concept of a "workspace". That is, the directories a user has
//! open in their editor, the files in them, the files they're currently editing, and so on.

use comemo::Prehashed;
use tower_lsp::lsp_types::{InitializeParams, TextDocumentContentChangeEvent, Url};
use typst::diag::FileResult;
use typst::eval::Library;
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use crate::config::PositionEncoding;
use crate::ext::InitializeParamsExt;

use self::font_manager::FontManager;
use self::fs::manager::FsManager;
use self::fs::FsProvider;
use self::project::manager::ProjectManager;
use self::project::ProjectMeta;

pub mod font_manager;
pub mod fs;
pub mod project;

pub struct Workspace {
    fs: FsManager,
    fonts: FontManager,
    projects: ProjectManager,

    // Needed so that `Workspace` can implement Typst's `World` trait
    pub typst_stdlib: Prehashed<Library>,
}

impl Workspace {
    pub fn new(params: &InitializeParams) -> Self {
        let root_paths = params.root_paths();

        Self {
            fs: FsManager::default(),
            fonts: FontManager::builder().with_system().with_embedded().build(),
            projects: ProjectManager::new(root_paths),
            typst_stdlib: Prehashed::new(typst_library::build()),
        }
    }

    pub fn font_manager(&self) -> &FontManager {
        &self.fonts
    }

    pub fn read_bytes(&self, uri: &Url) -> FileResult<Bytes> {
        self.fs.read_bytes(uri)
    }

    pub fn read_source(&self, uri: &Url) -> FileResult<Source> {
        self.fs.read_source(uri, &self.projects)
    }

    pub fn uri_to_project_and_id(
        &self,
        uri: &Url,
    ) -> FileResult<(Box<dyn ProjectMeta + Send + Sync>, FileId)> {
        self.projects.uri_to_project_and_id(uri)
    }

    pub fn open_lsp(&mut self, uri: Url, text: String) -> FileResult<()> {
        self.fs.open_lsp(uri, text, &self.projects)
    }

    pub fn close_lsp(&mut self, uri: &Url) {
        self.fs.close_lsp(uri)
    }

    pub fn edit_lsp(
        &mut self,
        uri: &Url,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
        position_encoding: PositionEncoding,
    ) {
        self.fs.edit_lsp(uri, changes, position_encoding)
    }

    pub fn invalidate_local(&mut self, uri: &Url) {
        self.fs.invalidate_local(uri)
    }

    pub fn clear(&mut self) {
        self.fonts.clear();
        self.fs.clear();
    }
}
