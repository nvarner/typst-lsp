use std::fs;
use std::path::Path;

use elsa::sync::FrozenMap;
use once_cell::sync::OnceCell;
use tower_lsp::lsp_types::Url;
use typst::diag::{FileError, FileResult};
use typst::syntax::Source;
use typst::util::Bytes;

use crate::lsp_typst_boundary::uri_to_path;
use crate::workspace::project::manager::ProjectManager;

use super::FsProvider;

/// Implements the Typst filesystem on the local filesystem, mapping Typst files to local files, and
/// providing conversions using [`Path`]s as an intermediate.
///
/// In this context, a "path" refers to an absolute path in the local filesystem. Paths in the Typst
/// filesystem are absolute, relative to either the project or some package. They use the same type,
/// but are meaningless when interpreted as local paths without accounting for the project or
/// package root. So, for consistency, we avoid using these Typst paths and prefer filesystem paths.
#[derive(Default)]
pub struct LocalFs {}

impl FsProvider for LocalFs {
    type Error = FileError;

    fn read_bytes(&self, uri: &Url) -> FileResult<Bytes> {
        let path = uri_to_path(uri)?;
        Self::read_path_raw(&path).map(Bytes::from)
    }

    fn read_source(&self, uri: &Url, project_manager: &ProjectManager) -> FileResult<Source> {
        let path = uri_to_path(uri)?;

        let extension_is_typ = || path.extension().map(|ext| ext == "typ").unwrap_or(false);
        if !extension_is_typ() {
            return Err(FileError::NotSource);
        };

        let raw = Self::read_path_raw(&path)?;

        let id = project_manager.uri_to_id(uri)?;
        let text = String::from_utf8(raw).map_err(|_| FileError::InvalidUtf8)?;
        Ok(Source::new(id, text))
    }
}

impl LocalFs {
    /// Regular read from filesystem, returning a [`FileResult`] on failure
    pub fn read_path_raw(path: &Path) -> FileResult<Vec<u8>> {
        fs::read(path).map_err(|err| FileError::from_io(err, path))
    }
}

#[derive(Default)]
pub struct LocalFsCache {
    entries: FrozenMap<Url, Box<CacheEntry>>,
    fs: LocalFs,
}

impl FsProvider for LocalFsCache {
    type Error = FileError;

    fn read_bytes(&self, uri: &Url) -> FileResult<Bytes> {
        self.read_bytes_ref(uri).cloned()
    }

    fn read_source(&self, uri: &Url, project_manager: &ProjectManager) -> FileResult<Source> {
        self.read_source_ref(uri, project_manager).cloned()
    }
}

impl LocalFsCache {
    pub fn read_bytes_ref(&self, uri: &Url) -> FileResult<&Bytes> {
        self.entry(uri.clone()).read_bytes(uri, &self.fs)
    }

    pub fn read_source_ref(
        &self,
        uri: &Url,
        project_manager: &ProjectManager,
    ) -> FileResult<&Source> {
        self.entry(uri.clone())
            .read_source(uri, &self.fs, project_manager)
    }

    pub fn cache_new(&mut self, uri: &Url) {
        self.entry_mut(uri.clone());
    }

    pub fn invalidate(&mut self, uri: &Url) {
        self.entry_mut(uri.clone()).invalidate()
    }

    pub fn delete(&mut self, uri: &Url) {
        self.entries.as_mut().remove(uri);
    }

    pub fn clear(&mut self) {
        self.entries.as_mut().clear()
    }

    fn entry(&self, uri: Url) -> &CacheEntry {
        self.entries
            .get(&uri) // don't take write lock unnecessarily
            .unwrap_or_else(|| self.entries.insert(uri, Box::default()))
    }

    fn entry_mut(&mut self, uri: Url) -> &mut CacheEntry {
        self.entries.as_mut().entry(uri).or_default()
    }
}

#[derive(Default)]
pub struct CacheEntry {
    source: OnceCell<Source>,
    bytes: OnceCell<Bytes>,
}

impl CacheEntry {
    pub fn read_bytes(&self, uri: &Url, fs: &LocalFs) -> FileResult<&Bytes> {
        self.bytes.get_or_try_init(|| fs.read_bytes(uri))
    }

    pub fn read_source(
        &self,
        uri: &Url,
        fs: &LocalFs,
        project_manager: &ProjectManager,
    ) -> FileResult<&Source> {
        self.source
            .get_or_try_init(|| fs.read_source(uri, project_manager))
    }

    pub fn invalidate(&mut self) {
        self.source.take();
        self.bytes.take();
    }
}
