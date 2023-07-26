use std::fs;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;
use typst::syntax::Source;
use typst::util::Bytes;

use crate::ext::PathExt;
use crate::workspace::project::manager::ProjectManager;

use super::{FsError, FsResult, ReadProvider, WriteProvider};

/// Implements the Typst filesystem on the local filesystem, mapping Typst files to local files, and
/// providing conversions using [`Path`]s as an intermediate.
///
/// In this context, a "path" refers to an absolute path in the local filesystem. Paths in the Typst
/// filesystem are absolute, relative to either the project or some package. They use the same type,
/// but are meaningless when interpreted as local paths without accounting for the project or
/// package root. So, for consistency, we avoid using these Typst paths and prefer filesystem paths.
#[derive(Default)]
pub struct LocalFs {}

impl ReadProvider for LocalFs {
    fn read_bytes(&self, uri: &Url) -> FsResult<Bytes> {
        let path = Self::uri_to_path(uri)?;
        Self::read_path_raw(&path).map(Bytes::from)
    }

    fn read_source(&self, uri: &Url, project_manager: &ProjectManager) -> FsResult<Source> {
        let path = Self::uri_to_path(uri)?;

        if !path.is_typst() {
            return Err(FsError::NotSource);
        }

        let text = Self::read_path_string(&path)?;
        let id = project_manager.uri_to_id(uri)?;
        Ok(Source::new(id, text))
    }
}

impl WriteProvider for LocalFs {
    fn write_raw(&self, uri: &Url, data: &[u8]) -> FsResult<()> {
        let path = Self::uri_to_path(uri)?;
        Self::write_path_raw(&path, data)
    }
}

impl LocalFs {
    pub fn uri_to_path(uri: &Url) -> Result<PathBuf, UriToPathError> {
        Self::verify_local(uri)?
            .to_file_path()
            .map_err(|()| UriToPathError::Conversion)
    }

    fn verify_local(uri: &Url) -> Result<&Url, UriToPathError> {
        if uri.scheme() == "file" {
            Ok(uri)
        } else {
            Err(UriToPathError::SchemeIsNotFile)
        }
    }

    pub fn path_to_uri(path: &Path) -> Result<Url, PathToUriError> {
        Url::from_file_path(path).map_err(|()| PathToUriError::NotAbsolute)
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
pub enum UriToPathError {
    #[error("cannot convert to path since scheme of URI is not `file`")]
    SchemeIsNotFile,
    #[error("URI to path conversion error")]
    Conversion,
}

#[derive(thiserror::Error, Debug)]
pub enum PathToUriError {
    #[error("cannot convert to URI since path is not absolute")]
    NotAbsolute,
}
