//! Holds types relating to the LSP concept of a "workspace". That is, the directories a user has
//! open in their editor, the files in them, the files they're currently editing, and so on.

use comemo::Prehashed;
use tower_lsp::lsp_types::{
    InitializeParams, TextDocumentContentChangeEvent, Url, WorkspaceFoldersChangeEvent,
};
use tracing::trace;
use typst::diag::FileResult;
use typst::eval::Library;
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use crate::config::PositionEncoding;
use crate::ext::InitializeParamsExt;

use self::font_manager::FontManager;
use self::fs::manager::FsManager;
use self::fs::{ReadProvider, WriteProvider};
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

    pub fn register_files(&mut self) {
        let uris = self.projects.find_source_uris();

        for uri in uris {
            trace!(%uri, "registering file");
            self.new_local(&uri);
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

    /// Write raw data to a file.
    ///
    /// This can cause cache invalidation errors if `uri` refers to a file in the cache, since the
    /// cache wouldn't know about the update. However, this is hard to fix, because we don't have
    /// `&mut self`.
    ///
    /// For example, when writing a PDF, we (effectively) have `&Workspace` after compiling via
    /// Typst, and we'd rather not lock everything just to export the PDF. However, if we allow for
    /// mutating files stored in the `Cache`, we could update a file while it is being used for a
    /// Typst compilation, which is also bad.
    pub fn write_raw(&self, uri: &Url, data: &[u8]) -> FileResult<()> {
        self.fs.write_raw(uri, data)
    }

    pub fn uri_to_project_and_id(&self, uri: &Url) -> FileResult<(Box<dyn ProjectMeta>, FileId)> {
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

    pub fn new_local(&mut self, uri: &Url) {
        self.fs.new_local(uri)
    }

    pub fn invalidate_local(&mut self, uri: &Url) {
        self.fs.invalidate_local(uri)
    }

    pub fn delete_local(&mut self, uri: &Url) {
        self.fs.delete_local(uri)
    }

    pub fn handle_workspace_folders_change_event(&mut self, event: &WorkspaceFoldersChangeEvent) {
        self.projects.handle_change_event(event);

        // The canonical project/id of URIs might have changed, so we need to invalidate the cache
        self.clear();
    }

    pub fn clear(&mut self) {
        self.fonts.clear();
        self.fs.clear();
        self.register_files();
    }
}
