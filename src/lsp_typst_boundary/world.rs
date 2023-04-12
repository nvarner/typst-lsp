use comemo::Prehashed;
use tokio::sync::OwnedRwLockWriteGuard;
use typst::diag::{FileError, FileResult};
use typst::eval::Library;
use typst::font::{Font, FontBook};
use typst::util::Buffer;
use typst::World;

use crate::workspace::source_manager::SourceId;
use crate::workspace::Workspace;

use super::{typst_to_lsp, TypstPath, TypstSource, TypstSourceId};

pub struct WorkspaceWorld {
    workspace: OwnedRwLockWriteGuard<Workspace>,
    main: SourceId,
}

impl WorkspaceWorld {
    pub fn new(workspace: OwnedRwLockWriteGuard<Workspace>, main: SourceId) -> Self {
        Self { workspace, main }
    }

    pub fn get_workspace(&self) -> &OwnedRwLockWriteGuard<Workspace> {
        &self.workspace
    }
}

impl World for WorkspaceWorld {
    fn library(&self) -> &Prehashed<Library> {
        &self.workspace.typst_stdlib
    }

    fn main(&self) -> &TypstSource {
        self.source(self.main.into())
    }

    fn resolve(&self, typst_path: &TypstPath) -> FileResult<TypstSourceId> {
        let lsp_uri = typst_to_lsp::path_to_uri(typst_path).unwrap();
        let lsp_id = self.workspace.sources.get_id_by_uri(&lsp_uri);
        match lsp_id {
            Some(lsp_id) => Ok(lsp_id.into()),
            None => Err(FileError::NotFound(typst_path.to_owned())),
        }
    }

    fn source(&self, typst_id: TypstSourceId) -> &TypstSource {
        let lsp_source = self.workspace.sources.get_source_by_id(typst_id.into());
        lsp_source.as_ref()
    }

    fn book(&self) -> &Prehashed<FontBook> {
        self.workspace.fonts.book()
    }

    fn font(&self, id: usize) -> Option<Font> {
        let mut resources = self.workspace.resources.write();
        self.workspace.fonts.font(id, &mut resources)
    }

    fn file(&self, typst_path: &TypstPath) -> FileResult<Buffer> {
        let lsp_uri = typst_to_lsp::path_to_uri(typst_path).unwrap();
        let mut resources = self.workspace.resources.write();
        let lsp_resource = resources.get_or_insert_resource(lsp_uri)?;
        Ok(lsp_resource.into())
    }
}
