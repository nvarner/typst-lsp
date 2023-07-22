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

use self::font_manager::FontManager;
use self::fs::local::{LocalFs, LocalFsCache};
use self::fs::lsp::LspFs;
use self::fs::manager::FsManager;
use self::fs::FsProvider;

pub mod font_manager;
pub mod fs;
pub mod project;

pub struct Workspace {
    fs_manager: FsManager,
    fonts: FontManager,

    // Needed so that `Workspace` can implement Typst's `World` trait
    pub typst_stdlib: Prehashed<Library>,
}

impl Workspace {
    #[allow(deprecated)] // `params.root_path` is marked as deprecated
    pub fn new(params: &InitializeParams) -> Self {
        // TODO: multi-root workspaces
        let project_root = params
            .root_uri
            .as_ref()
            .and_then(|uri| uri.to_file_path().ok())
            .or_else(|| params.root_path.as_ref()?.try_into().ok())
            .expect("could not get project root");

        Self {
            fs_manager: FsManager::new(
                LspFs::default(),
                LocalFsCache::new(LocalFs::new(project_root)),
            ),
            fonts: FontManager::builder().with_system().with_embedded().build(),
            typst_stdlib: Prehashed::new(typst_library::build()),
        }
    }

    pub fn fs_manager(&self) -> &FsManager {
        &self.fs_manager
    }

    pub fn font_manager(&self) -> &FontManager {
        &self.fonts
    }

    pub fn read_file(&self, id: FileId) -> FileResult<Bytes> {
        self.fs_manager.read_bytes(id)
    }

    pub fn read_source(&self, id: FileId) -> FileResult<Source> {
        self.fs_manager.read_source(id)
    }

    pub fn uri_to_id(&self, uri: &Url) -> FileResult<FileId> {
        self.fs_manager.uri_to_id(uri)
    }

    pub fn id_to_uri(&self, id: FileId) -> FileResult<Url> {
        self.fs_manager.id_to_uri(id)
    }

    pub fn open_lsp(&mut self, id: FileId, text: String) {
        self.fs_manager.open_lsp(id, text);
    }

    pub fn close_lsp(&mut self, id: FileId) {
        self.fs_manager.close_lsp(id)
    }

    pub fn edit_lsp(
        &mut self,
        id: FileId,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
        position_encoding: PositionEncoding,
    ) {
        self.fs_manager.edit_lsp(id, changes, position_encoding)
    }

    pub fn invalidate_local(&mut self, id: FileId) {
        self.fs_manager.invalidate_local(id)
    }

    pub fn clear(&mut self) {
        self.fonts.clear();
        self.fs_manager.clear();
    }
}
