use std::fs;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;
use typst::eval::Bytes;
use typst::syntax::Source;
use walkdir::WalkDir;

use crate::ext::PathExt;
use crate::workspace::package::manager::PackageManager;

use super::{FsError, FsResult, ReadProvider, SourceSearcher, WriteProvider};

/// Implements the Typst filesystem on the local filesystem, mapping Typst files to local files, and
/// providing conversions using [`Path`]s as an intermediate.
///
/// In this context, a "path" refers to an absolute path in the local filesystem. Paths in the Typst
/// filesystem are absolute, relative to either the project or some package. They use the same type,
/// but are meaningless when interpreted as local paths without accounting for the project or
/// package root. So, for consistency, we avoid using these Typst paths and prefer filesystem paths.
#[derive(Debug, Default)]
pub struct LocalFs {}

impl ReadProvider for LocalFs {
    fn read_bytes(&self, uri: &Url, _: &PackageManager) -> FsResult<Bytes> {
        let path = Self::uri_to_path(uri)?;
        Self::read_path_raw(&path).map(Bytes::from)
    }

    fn read_source(&self, uri: &Url, package_manager: &PackageManager) -> FsResult<Source> {
        let path = Self::uri_to_path(uri)?;

        if !path.is_typst() {
            return Err(FsError::NotSource);
        }

        let text = Self::read_path_string(&path)?;
        let full_id = package_manager.full_id(uri)?;
        Ok(Source::new(full_id.into(), text))
    }
}

impl WriteProvider for LocalFs {
    fn write_raw(&self, uri: &Url, data: &[u8]) -> FsResult<()> {
        let path = Self::uri_to_path(uri)?;
        Self::write_path_raw(&path, data)
    }
}

impl SourceSearcher for LocalFs {
    fn search_sources(&self, root: &Url) -> FsResult<Vec<Url>> {
        let path = Self::uri_to_path(root)?;

        let sources = WalkDir::new(path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter(|file| file.path().is_typst())
            .map(|file| {
                LocalFs::path_to_uri(file.path())
                    .expect("path should be absolute since walkdir was given an absolute path")
            })
            .collect();

        Ok(sources)
    }
}

impl LocalFs {
    pub fn uri_to_path(uri: &Url) -> Result<PathBuf, UriToFsPathError> {
        Self::verify_local(uri)?
            .to_file_path()
            .map_err(|()| UriToFsPathError::Conversion)
    }

    fn verify_local(uri: &Url) -> Result<&Url, UriToFsPathError> {
        if uri.scheme() == "file" {
            Ok(uri)
        } else {
            Err(UriToFsPathError::SchemeIsNotFile)
        }
    }

    pub fn path_to_uri(path: impl AsRef<Path>) -> Result<Url, FsPathToUriError> {
        Url::from_file_path(path).map_err(|()| FsPathToUriError::NotAbsolute)
    }

    /// Regular read from filesystem, returning a [`FileResult`] on failure
    pub fn read_path_raw(path: &Path) -> FsResult<Vec<u8>> {
        fs::read(path).map_err(|err| FsError::from_local_io(err, path))
    }

    pub fn read_path_string(path: &Path) -> FsResult<String> {
        fs::read_to_string(path).map_err(|err| FsError::from_local_io(err, path))
    }

    pub fn write_path_raw(path: &Path, data: &[u8]) -> FsResult<()> {
        fs::write(path, data).map_err(|err| FsError::from_local_io(err, path))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum UriToFsPathError {
    #[error("cannot convert to path since scheme of URI is not `file`")]
    SchemeIsNotFile,
    #[error("URI to path conversion error")]
    Conversion,
}

#[derive(thiserror::Error, Debug)]
pub enum FsPathToUriError {
    #[error("cannot convert to URI since path is not absolute")]
    NotAbsolute,
}
