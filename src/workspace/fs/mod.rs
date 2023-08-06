use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;
use tracing::error;
use typst::diag::FileError;
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use crate::ext::UriError;

use self::local::UriToFsPathError;

use super::package::manager::{PackageError, PackageManager};

pub mod cache;
pub mod local;
pub mod lsp;
pub mod manager;

/// Read access to the Typst filesystem for a single workspace
pub trait ReadProvider {
    fn read_bytes(&self, uri: &Url, package_manager: &PackageManager) -> FsResult<Bytes>;
    fn read_source(&self, uri: &Url, package_manager: &PackageManager) -> FsResult<Source>;
}

/// Write access to the Typst filesystem for a single workspace
pub trait WriteProvider {
    fn write_raw(&self, uri: &Url, data: &[u8]) -> FsResult<()>;
}

pub trait SourceSearcher {
    fn search_sources(&self, root: &Url) -> FsResult<Vec<Url>>;
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
    Package(#[from] PackageError),
    #[error(transparent)]
    OtherIo(io::Error),
    #[error("the provider does not provide the requested URI")]
    NotProvided(#[source] anyhow::Error),
    #[error("could not join path to URI")]
    UriJoin(#[from] UriError),
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
            Self::Package(err) => err.convert(id),
            Self::OtherIo(err) => FileError::from_io(err, id.path()),
            Self::NotProvided(_) | Self::UriJoin(_) | Self::Other(_) => FileError::Other,
        }
    }
}

impl From<UriToFsPathError> for FsError {
    fn from(err: UriToFsPathError) -> Self {
        match err {
            UriToFsPathError::SchemeIsNotFile => Self::NotProvided(err.into()),
            UriToFsPathError::Conversion => Self::Other(err.into()),
        }
    }
}
