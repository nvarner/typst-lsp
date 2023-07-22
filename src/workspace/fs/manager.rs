use tower_lsp::lsp_types::{TextDocumentContentChangeEvent, Url};
use typst::diag::{FileError, FileResult};
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use crate::config::PositionEncoding;

use super::local::LocalFsCache;
use super::lsp::LspFs;
use super::FsProvider;

/// Composes [`FsProvider`]s into a single provider for a workspace.
pub struct FsManager {
    lsp: LspFs,
    local: LocalFsCache,
}

impl FsManager {
    pub fn new(lsp: LspFs, local: LocalFsCache) -> Self {
        Self { lsp, local }
    }

    pub fn open_lsp(&mut self, id: FileId, text: String) {
        self.lsp.open(id, text);
    }

    pub fn close_lsp(&mut self, id: FileId) {
        self.lsp.close(id)
    }

    pub fn edit_lsp(
        &mut self,
        id: FileId,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
        position_encoding: PositionEncoding,
    ) {
        self.lsp.edit(id, changes, position_encoding)
    }

    pub fn invalidate_local(&mut self, id: FileId) {
        self.local.invalidate(id)
    }

    pub fn clear(&mut self) {
        self.lsp.clear();
        self.local.clear();
    }
}

impl FsProvider for FsManager {
    type Error = FileError;

    fn read_bytes(&self, uri: &Url) -> FileResult<Bytes> {
        self.lsp
            .read_bytes(uri)
            .or_else(|()| self.local.read_bytes(uri))
    }

    fn read_source(&self, uri: &Url) -> FileResult<Source> {
        self.lsp
            .read_source(uri)
            .or_else(|()| self.local.read_source(uri))
    }
}
