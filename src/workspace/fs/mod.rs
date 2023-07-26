use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;
use tracing::error;
use typst::diag::FileError;
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use self::local::UriToPathError;

use super::project::manager::ProjectManager;
use super::project::IdToUriError;

pub mod cache;
pub mod local;
pub mod lsp;
pub mod manager;

/// Read access to the Typst filesystem for a single workspace
pub trait ReadProvider {
    fn read_bytes(&self, uri: &Url) -> FsResult<Bytes>;
    fn read_source(&self, uri: &Url, project_manager: &ProjectManager) -> FsResult<Source>;
}

/// Write access to the Typst filesystem for a single workspace
pub trait WriteProvider {
    fn write_raw(&self, uri: &Url, data: &[u8]) -> FsResult<()>;
}

/// Remembers URIs if available sources
pub trait KnownUriProvider {
    fn known_uris(&self) -> HashSet<Url>;
}

pub type FsResult<T> = Result<T, FsError>;

#[derive(thiserror::Error, Debug)]
pub enum FsError {
    #[error("expected Typst source file, but found something else")]
    NotSource,
    #[error("could not find `{0}` on the local filesystem")]
    NotFoundLocal(PathBuf),
    #[error(transparent)]
    OtherIo(io::Error),
    #[error("the provider does not provide the requested URI")]
    NotProvided(#[source] anyhow::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl FsError {
    pub fn from_local_io(err: io::Error, local_path: &Path) -> Self {
        match err.kind() {
            io::ErrorKind::NotFound => Self::NotFoundLocal(local_path.to_owned()),
            _ => Self::OtherIo(err),
        }
    }

    pub fn report_and_convert(self, id: FileId) -> FileError {
        error!(err = %self, "filesystem error");
        match self {
            Self::NotSource => FileError::NotSource,
            Self::NotFoundLocal(path) => FileError::NotFound(path),
            Self::OtherIo(err) => FileError::from_io(err, id.path()),
            Self::NotProvided(_) | Self::Other(_) => FileError::Other,
        }
    }
}

impl From<UriToPathError> for FsError {
    fn from(err: UriToPathError) -> Self {
        match err {
            UriToPathError::SchemeIsNotFile => Self::NotProvided(err.into()),
            UriToPathError::Conversion => Self::Other(err.into()),
        }
    }
}

impl From<IdToUriError> for FsError {
    fn from(err: IdToUriError) -> Self {
        match err {
            IdToUriError::Other(err) => Self::Other(err),
        }
    }
}
