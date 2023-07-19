//! Holds types relating to the LSP concept of a "workspace". That is, the directories a user has
//! open in their editor, the files in them, the files they're currently editing, and so on.

use comemo::Prehashed;
use tower_lsp::lsp_types::{InitializeParams, Url};
use typst::diag::FileResult;
use typst::eval::Library;
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use self::font_manager::FontManager;
use self::fs::local::{FsLocalCache, LocalFs};
use self::fs::FsProvider;

pub mod font_manager;
pub mod fs;
pub mod source_manager;

pub struct Workspace {
    fs: FsLocalCache,
    fonts: FontManager,

    // Needed so that `Workspace` can implement Typst's `World` trait
    pub typst_stdlib: Prehashed<Library>,
}

impl Workspace {
    #[allow(deprecated)] // `params.root_path` is marked as deprecated
    pub fn new(params: InitializeParams) -> Self {
        // TODO: multi-root workspaces
        let project_root = params
            .root_uri
            .and_then(|uri| uri.to_file_path().ok())
            .or_else(|| params.root_path?.try_into().ok())
            .expect("could not get project root");

        Self {
            fs: FsLocalCache::new(LocalFs::new(project_root)),
            fonts: FontManager::builder().with_system().with_embedded().build(),
            typst_stdlib: Prehashed::new(typst_library::build()),
        }
    }

    // pub fn source_manager(&self) -> &impl SourceManager {
    //     &self.files
    // }

    // pub fn source_manager_mut(&mut self) -> &mut impl SourceManager {
    //     &mut self.files
    // }

    pub fn font_manager(&self) -> &FontManager {
        &self.fonts
    }

    pub fn read_file(&self, id: FileId) -> FileResult<Bytes> {
        self.fs.read_bytes(id)
    }

    pub fn read_source(&self, id: FileId) -> FileResult<Source> {
        self.fs.read_source(id)
    }

    pub fn uri_to_id(&self, uri: &Url) -> FileResult<FileId> {
        self.fs.uri_to_id(uri)
    }

    pub fn id_to_uri(&self, id: FileId) -> FileResult<Url> {
        self.fs.id_to_uri(id)
    }

    pub fn invalidate(&mut self, id: FileId) {
        self.fs.invalidate(id)
    }

    pub fn clear(&mut self) {
        self.fonts.clear();
        self.fs.clear();
    }
}
