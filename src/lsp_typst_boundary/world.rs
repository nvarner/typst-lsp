use comemo::Prehashed;
use tokio::sync::OwnedRwLockReadGuard;
use typst::diag::FileResult;
use typst::eval::Library;
use typst::font::{Font, FontBook};
use typst::util::Buffer;
use typst::World;

use crate::workspace::source_manager::SourceId;
use crate::workspace::Workspace;

use super::{typst_to_lsp, TypstPath, TypstSource, TypstSourceId};

pub struct WorkspaceWorld {
    workspace: OwnedRwLockReadGuard<Workspace>,
    main: SourceId,
}

impl WorkspaceWorld {
    pub fn new(workspace: OwnedRwLockReadGuard<Workspace>, main: SourceId) -> Self {
        Self { workspace, main }
    }

    pub fn get_workspace(&self) -> &OwnedRwLockReadGuard<Workspace> {
        &self.workspace
    }
}

impl World for WorkspaceWorld {
    fn library(&self) -> &Prehashed<Library> {
        let workspace = self.get_workspace();
        &workspace.typst_stdlib
    }

    fn main(&self) -> &TypstSource {
        self.source(self.main.into())
    }

    fn resolve(&self, typst_path: &TypstPath) -> FileResult<TypstSourceId> {
        let lsp_uri = typst_to_lsp::path_to_uri(typst_path).unwrap();
        self.get_workspace().sources.cache(lsp_uri).map(Into::into)
    }

    fn source(&self, typst_id: TypstSourceId) -> &TypstSource {
        let lsp_source = self
            .get_workspace()
            .sources
            .get_open_source_by_id(typst_id.into());
        lsp_source.as_ref()
    }

    fn book(&self) -> &Prehashed<FontBook> {
        self.get_workspace().fonts.book()
    }

    fn font(&self, id: usize) -> Option<Font> {
        let mut resources = self.get_workspace().resources.write();
        self.get_workspace().fonts.font(id, &mut resources)
    }

    fn file(&self, typst_path: &TypstPath) -> FileResult<Buffer> {
        let lsp_uri = typst_to_lsp::path_to_uri(typst_path).unwrap();
        let mut resources = self.get_workspace().resources.write();
        let lsp_resource = resources.get_or_insert_resource(lsp_uri)?;
        Ok(lsp_resource.into())
    }
}
