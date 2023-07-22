use tower_lsp::lsp_types::{TextDocumentContentChangeEvent, Url};
use typst::diag::{FileError, FileResult};
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use crate::config::PositionEncoding;

use super::local::LocalFsCache;
use super::lsp::LspFs;
use super::FsProvider;

/// Composes [`FsProvider`](super::FsProvider)s into a single provider for a Typst project.
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

    pub fn delete_local(&mut self, id: FileId) {
        self.local.delete(id)
    }

    pub fn clear(&mut self) {
        self.lsp.clear();
        self.local.clear();
    }
}

impl FsProvider for FsManager {
    type Error = FileError;

    fn read_raw(&self, id: FileId) -> FileResult<Vec<u8>> {
        self.lsp.read_raw(id).or_else(|()| self.local.read_raw(id))
    }

    fn read_bytes(&self, id: FileId) -> FileResult<Bytes> {
        self.lsp
            .read_bytes(id)
            .or_else(|()| self.local.read_bytes(id))
    }

    fn read_source(&self, id: FileId) -> FileResult<Source> {
        self.lsp
            .read_source(id)
            .or_else(|()| self.local.read_source(id))
    }

    fn uri_to_id(&self, uri: &Url) -> FileResult<FileId> {
        self.lsp
            .uri_to_id(uri)
            .or_else(|()| self.local.uri_to_id(uri))
    }

    fn id_to_uri(&self, id: FileId) -> FileResult<Url> {
        self.lsp
            .id_to_uri(id)
            .or_else(|()| self.local.id_to_uri(id))
    }
}
