use std::fs;
use std::path::{Path, PathBuf};

use elsa::sync::FrozenMap;
use once_cell::sync::OnceCell;
use tower_lsp::lsp_types::Url;
use tracing::error;
use typst::diag::{FileError, FileResult};
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use super::FsProvider;

/// Implements the Typst filesystem on the local filesystem, mapping Typst files to local files, and
/// providing conversions using [`Path`]s as an intermediate.
///
/// In this context, a "path" refers to an absolute path in the local filesystem. Paths in the Typst
/// filesystem are absolute, relative to either the project or some package. They use the same type,
/// but are meaningless when interpreted as local paths without accounting for the project or
/// package root. So, for consistency, we avoid using these Typst paths and prefer filesystem paths.
pub struct LocalFs {
    project_root: PathBuf,
}

impl FsProvider for LocalFs {
    type Error = FileError;

    fn read_bytes(&self, uri: &Url) -> FileResult<Bytes> {
        let path = self.uri_to_path(uri)?;
        Self::read_path_raw(&path).map(Bytes::from)
    }

    fn read_source(&self, uri: &Url) -> FileResult<Source> {
        let path = self.uri_to_path(uri)?;

        let extension_is_typ = || path.extension().map(|ext| ext == "typ").unwrap_or(false);
        if !extension_is_typ() {
            return Err(FileError::NotSource);
        };

        let raw = Self::read_path_raw(&path)?;

        let text = String::from_utf8(raw).map_err(|_| FileError::InvalidUtf8)?;
        Ok(Source::new(uri, text))
    }
}

impl LocalFs {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Regular read from filesystem, returning a [`FileResult`] on failure
    pub fn read_path_raw(path: &Path) -> FileResult<Vec<u8>> {
        fs::read(path).map_err(|err| FileError::from_io(err, path))
    }

    fn project_root(&self) -> &Path {
        &self.project_root
    }

    fn uri_to_path(&self, uri: &Url) -> FileResult<PathBuf> {
        let is_local = |uri: &Url| uri.scheme() == "file";
        let handle_not_local = || format!("URI scheme `{}` is not `file`", uri.scheme());
        let verify_local = |uri| is_local(uri).then_some(uri).ok_or_else(handle_not_local);

        let handle_make_local_error = |()| "could not convert URI to path".to_owned();
        let make_local = |uri: &Url| uri.to_file_path().map_err(handle_make_local_error);

        let handle_error = |err| {
            error!(%uri, message = err);
            FileError::Other
        };

        verify_local(uri).and_then(make_local).map_err(handle_error)
    }

    fn path_to_uri(&self, path: &Path) -> FileResult<Url> {
        let handle_error = |()| {
            error!(path = %path.display(), "could not convert path to URI");
            FileError::NotFound(path.to_owned())
        };

        Url::from_file_path(path).map_err(handle_error)
    }
}

pub struct LocalFsCache {
    entries: FrozenMap<Url, Box<CacheEntry>>,
    fs: LocalFs,
}

impl FsProvider for LocalFsCache {
    type Error = FileError;

    fn read_bytes(&self, uri: &Url) -> FileResult<Bytes> {
        self.read_bytes_ref(uri).cloned()
    }

    fn read_source(&self, uri: &Url) -> FileResult<Source> {
        self.read_source_ref(uri).cloned()
    }
}

impl LocalFsCache {
    pub fn new(fs: LocalFs) -> Self {
        Self {
            entries: Default::default(),
            fs,
        }
    }

    pub fn read_bytes_ref(&self, uri: &Url) -> FileResult<&Bytes> {
        self.entry(uri.clone()).read_bytes(uri, &self.fs)
    }

    pub fn read_source_ref(&self, uri: &Url) -> FileResult<&Source> {
        self.entry(uri.clone()).read_source(uri, &self.fs)
    }

    pub fn invalidate(&mut self, uri: &Url) {
        self.entries.as_mut().remove(uri);
    }

    pub fn clear(&mut self) {
        self.entries.as_mut().clear()
    }

    fn entry(&self, id: FileId) -> &CacheEntry {
        self.entries
            .get(&id) // don't take write lock unnecessarily
            .unwrap_or_else(|| self.entries.insert(id, Box::default()))
    }
}

#[derive(Default)]
pub struct CacheEntry {
    source: OnceCell<Source>,
    bytes: OnceCell<Bytes>,
}

impl CacheEntry {
    pub fn read_source(&self, uri: &Url, fs: &LocalFs) -> FileResult<&Source> {
        self.source.get_or_try_init(|| fs.read_source(uri))
    }

    pub fn read_bytes(&self, uri: &Url, fs: &LocalFs) -> FileResult<&Bytes> {
        self.bytes.get_or_try_init(|| fs.read_bytes(uri))
    }
}
