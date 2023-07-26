//! Holds types related to Typst projects. A [`Project`] lives on top of a [`Workspace`], and is the
//! largest scope in which arbitrary [`FileId`]s make sense, since we otherwise don't know what
//! package an ID of the form `(None, _)` refers to.

use std::fmt;
use std::ops::Deref;

use tokio::sync::OwnedRwLockReadGuard;
use tower_lsp::lsp_types::Url;
use tracing::error;
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use crate::workspace::Workspace;

use super::fs::local::{PathToUriError, UriToPathError};
use super::fs::FsResult;

pub mod local;
pub mod manager;

pub struct Project {
    workspace: OwnedRwLockReadGuard<Workspace>,
    meta: Box<dyn ProjectMeta>,
}

impl fmt::Debug for Project {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Project").field(&self.meta).finish()
    }
}

impl Project {
    pub fn new(workspace: OwnedRwLockReadGuard<Workspace>, meta: Box<dyn ProjectMeta>) -> Self {
        Self { workspace, meta }
    }

    pub fn workspace(&self) -> &Workspace {
        self.workspace.deref()
    }

    pub fn read_bytes(&self, id: FileId) -> FsResult<Bytes> {
        let uri = self.meta.id_to_uri(id)?;
        self.workspace().read_bytes(&uri)
    }

    pub fn read_source(&self, id: FileId) -> FsResult<Source> {
        let uri = self.meta.id_to_uri(id)?;
        self.workspace().read_source(&uri)
    }

    pub fn write_raw(&self, id: FileId, data: &[u8]) -> FsResult<()> {
        let uri = self.meta.id_to_uri(id)?;
        self.workspace().write_raw(&uri, data)
    }
}

pub trait ProjectMeta: Send + Sync + fmt::Debug {
    fn uri_to_id(&self, uri: &Url) -> Result<FileId, UriToIdError>;
    fn id_to_uri(&self, id: FileId) -> Result<Url, IdToUriError>;
}

#[derive(thiserror::Error, Debug)]
pub enum UriToIdError {
    #[error("cannot convert to ID since URI is not in the described project")]
    NotInProject,
    #[error(transparent)]
    Other(anyhow::Error),
}

impl From<UriToPathError> for UriToIdError {
    fn from(err: UriToPathError) -> Self {
        match err {
            UriToPathError::SchemeIsNotFile => Self::NotInProject,
            UriToPathError::Conversion => Self::Other(err.into()),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum IdToUriError {
    #[error(transparent)]
    Other(anyhow::Error),
}

impl From<PathToUriError> for IdToUriError {
    fn from(err: PathToUriError) -> Self {
        Self::Other(err.into())
    }
}
