use std::collections::HashSet;

use tower_lsp::lsp_types::{TextDocumentContentChangeEvent, Url};
use typst::syntax::Source;
use typst::util::Bytes;

use crate::config::PositionEncoding;
use crate::workspace::package::manager::PackageManager;

use super::cache::Cache;
use super::local::LocalFs;
use super::lsp::LspFs;
use super::{FsResult, KnownUriProvider, ReadProvider, WriteProvider};

/// Composes [`ReadProvider`]s and [`WriteProvider`]s into a single provider for a workspace
#[derive(Debug, Default)]
pub struct FsManager {
    lsp: LspFs,
    local: Cache<LocalFs>,
}

impl ReadProvider for FsManager {
    fn read_bytes(&self, uri: &Url, package_manager: &PackageManager) -> FsResult<Bytes> {
        self.lsp
            .read_bytes(uri, package_manager)
            .or_else(|_| self.local.read_bytes(uri, package_manager))
    }

    fn read_source(&self, uri: &Url, package_manager: &PackageManager) -> FsResult<Source> {
        self.lsp
            .read_source(uri, package_manager)
            .or_else(|_| self.local.read_source(uri, package_manager))
    }
}

impl WriteProvider for FsManager {
    fn write_raw(&self, uri: &Url, data: &[u8]) -> FsResult<()> {
        self.local.inner().write_raw(uri, data)
    }
}

impl KnownUriProvider for FsManager {
    fn known_uris(&self) -> HashSet<Url> {
        let mut uris = self.local.known_uris();
        uris.extend(self.lsp.known_uris().into_iter());
        uris
    }
}

impl FsManager {
    #[tracing::instrument]
    pub fn register_files(&mut self, root: &Url) -> FsResult<()> {
        self.local.register_files(root)
    }

    pub fn open_lsp(
        &mut self,
        uri: Url,
        text: String,
        package_manager: &PackageManager,
    ) -> FsResult<()> {
        self.lsp.open(uri, text, package_manager)
    }

    pub fn close_lsp(&mut self, uri: &Url) {
        self.lsp.close(uri)
    }

    pub fn edit_lsp(
        &mut self,
        uri: &Url,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
        position_encoding: PositionEncoding,
    ) {
        self.lsp.edit(uri, changes, position_encoding)
    }

    pub fn new_local(&mut self, uri: Url) {
        self.local.cache_new(uri)
    }

    pub fn invalidate_local(&mut self, uri: Url) {
        self.local.invalidate(uri)
    }

    pub fn delete_local(&mut self, uri: &Url) {
        self.local.delete(uri)
    }

    pub fn clear(&mut self) {
        self.lsp.clear();
        self.local.clear();
    }
}
