use comemo::Prehashed;
use tokio::sync::OwnedRwLockReadGuard;
use tracing::{error, warn};
use typst::diag::{EcoString, FileResult};
use typst::eval::{Datetime, Library};
use typst::file::{FileId, PackageSpec};
use typst::font::{Font, FontBook};
use typst::syntax::Source;
use typst::util::Bytes;
use typst::World;

use crate::workspace::Workspace;

use super::clock::Now;

/// Short-lived struct to implement [`World`] for [`Workspace`]. It wraps a `Workspace` with a main
/// file and exists for the lifetime of a Typst invocation.
pub struct WorkspaceWorld {
    workspace: OwnedRwLockReadGuard<Workspace>,
    main: FileId,
    /// Current time. Will be cached lazily for consistency throughout a compilation.
    now: Now,
}

impl WorkspaceWorld {
    pub fn new(workspace: OwnedRwLockReadGuard<Workspace>, main: FileId) -> Self {
        Self {
            workspace,
            main,
            now: Now::new(),
        }
    }

    pub fn workspace(&self) -> &OwnedRwLockReadGuard<Workspace> {
        &self.workspace
    }
}

impl World for WorkspaceWorld {
    fn library(&self) -> &Prehashed<Library> {
        &self.workspace().typst_stdlib
    }

    fn book(&self) -> &Prehashed<FontBook> {
        self.workspace().font_manager().book()
    }

    fn main(&self) -> Source {
        let handle_no_main = |err| {
            error!(
                ?err,
                "this is a bug: failed to get main with id {}", self.main
            );
            warn!("returning fake main file");
            Source::detached("")
        };

        self.source(self.main).unwrap_or_else(handle_no_main)
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.workspace().read_source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.workspace().read_file(id)
    }

    fn font(&self, id: usize) -> Option<Font> {
        self.workspace().font_manager().font(id)
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        self.now.with_typst_offset(offset)
    }

    fn packages(&self) -> &[(PackageSpec, Option<EcoString>)] {
        // TODO: implement packages
        &[]
    }
}
