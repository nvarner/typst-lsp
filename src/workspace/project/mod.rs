//! Holds types related to Typst projects. A [`Project`] lives on top of a [`Workspace`], and is the
//! largest scope in which arbitrary [`FileId`]s make sense, since we otherwise don't know what
//! package an ID of the form `(None, _)` refers to.

use std::fmt;
use std::ops::Deref;

use tokio::sync::OwnedRwLockReadGuard;
use tower_lsp::lsp_types::Url;
use typst::diag::FileResult;
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use crate::workspace::Workspace;

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

    pub fn read_bytes(&self, id: FileId) -> FileResult<Bytes> {
        let uri = self.meta.id_to_uri(id)?;
        self.workspace().read_bytes(&uri)
    }

    pub fn read_source(&self, id: FileId) -> FileResult<Source> {
        let uri = self.meta.id_to_uri(id)?;
        self.workspace().read_source(&uri)
    }

    pub fn write_raw(&self, id: FileId, data: &[u8]) -> FileResult<()> {
        let uri = self.meta.id_to_uri(id)?;
        self.workspace().write_raw(&uri, data)
    }
}

pub trait ProjectMeta: Send + Sync + fmt::Debug {
    fn uri_to_id(&self, uri: &Url) -> FileResult<FileId>;
    fn id_to_uri(&self, id: FileId) -> FileResult<Url>;
}
