//! Holds types related to Typst projects. A [`Project`] lives on top of a [`Workspace`], and is the
//! largest scope in which arbitrary [`FileId`]s make sense, since we otherwise don't know what
//! package an ID of the form `(None, _)` refers to.

use tower_lsp::lsp_types::Url;
use typst::diag::FileResult;
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use crate::workspace::fs::FsProvider;
use crate::workspace::Workspace;

pub mod local;

pub struct Project<W: AsRef<Workspace>, C: ProjectConverter> {
    workspace: W,
    converter: C,
}

impl<W: AsRef<Workspace>, C: ProjectConverter> Project<W, C> {
    pub fn workspace(&self) -> &Workspace {
        self.workspace.as_ref()
    }

    pub fn read_bytes(&self, id: FileId) -> FileResult<Bytes> {
        let uri = self.converter.id_to_uri(id)?;
        self.workspace().fs_manager().read_bytes(&uri)
    }

    pub fn read_source(&self, id: FileId) -> FileResult<Source> {
        let uri = self.converter.id_to_uri(id)?;
        self.workspace().fs_manager().read_source(&uri)
    }
}

pub trait ProjectConverter {
    fn uri_to_id(&self, uri: &Url) -> FileResult<FileId>;
    fn id_to_uri(&self, id: FileId) -> FileResult<Url>;
}
