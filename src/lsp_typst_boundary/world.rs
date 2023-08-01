use comemo::Prehashed;
use futures::Future;
use tokio::runtime;
use tower_lsp::lsp_types::Url;
use tracing::error;
use typst::diag::{EcoString, FileError, FileResult};
use typst::eval::{Datetime, Library};
use typst::file::{FileId, PackageSpec};
use typst::font::{Font, FontBook};
use typst::syntax::Source;
use typst::util::Bytes;
use typst::World;

use crate::workspace::fs::{FsError, FsResult};
use crate::workspace::package::manager::PackageManager;
use crate::workspace::package::FullFileId;
use crate::workspace::project::Project;
use crate::workspace::Workspace;

use super::clock::Now;

/// Short-lived struct to implement [`World`] for [`Project`]. It wraps a `Project` with a main file
/// and exists for the lifetime of a Typst invocation.
#[derive(Debug)]
pub struct ProjectWorld {
    project: Project,
    main: Url,
    /// Current time. Will be cached lazily for consistency throughout a compilation.
    now: Now,
}

impl ProjectWorld {
    pub fn new(project: Project, main: Url) -> Self {
        Self {
            project,
            main,
            now: Now::new(),
        }
    }

    pub fn workspace(&self) -> &Workspace {
        self.project.workspace()
    }

    fn package_manager(&self) -> &PackageManager {
        self.workspace().package_manager()
    }

    pub fn fill_id(&self, id: FileId) -> FullFileId {
        self.project.fill_id(id)
    }

    pub async fn full_id_to_uri(&self, full_id: FullFileId) -> FsResult<Url> {
        let package = self.package_manager().package(full_id.package()).await?;
        let uri = package.path_to_uri(full_id.path())?;
        Ok(uri)
    }

    async fn read_source_by_id(&self, id: FileId) -> FsResult<Source> {
        let full_id = self.fill_id(id);
        let uri = self.full_id_to_uri(full_id).await?;
        let source = self.workspace().read_source(&uri)?;
        Ok(source)
    }

    async fn read_bytes_by_id(&self, id: FileId) -> FsResult<Bytes> {
        let full_id = self.fill_id(id);
        let uri = self.full_id_to_uri(full_id).await?;
        let bytes = self.workspace().read_bytes(&uri)?;
        Ok(bytes)
    }

    /// Runs a `Future` in a non-async function, blocking until completion
    ///
    /// `comemo` doesn't support async, so Typst can't, so we're stuck with this for now to run
    /// async code in the `World` implementation
    fn block<T>(fut: impl Future<Output = T>) -> T {
        let rt = runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(fut)
    }
}

impl World for ProjectWorld {
    fn library(&self) -> &Prehashed<Library> {
        &self.workspace().typst_stdlib
    }

    fn book(&self) -> &Prehashed<FontBook> {
        self.workspace().font_manager().book()
    }

    fn main(&self) -> Source {
        self.workspace()
            .read_source(&self.main)
            .expect("main should be chosen to exist when world is constructed")
    }

    #[tracing::instrument]
    fn source(&self, id: FileId) -> FileResult<Source> {
        Self::block(self.read_source_by_id(id)).map_err(|err: FsError| err.report_and_convert(id))
    }

    #[tracing::instrument]
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Self::block(self.read_bytes_by_id(id)).map_err(|err: FsError| err.report_and_convert(id))
    }

    fn font(&self, id: usize) -> Option<Font> {
        self.workspace().font_manager().font(id)
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        self.now.with_typst_offset(offset)
    }

    fn packages(&self) -> &[(PackageSpec, Option<EcoString>)] {
        // TODO: implement package completion
        &[]
    }
}

#[derive(thiserror::Error, Debug)]
pub enum IdToUriError {
    #[error("the ID's path escapes the root directory")]
    PathEscapesRoot,
}

impl IdToUriError {
    pub fn report_and_convert(self) -> FileError {
        error!(err = %self, "file ID to URI conversion error");

        match self {
            Self::PathEscapesRoot => FileError::AccessDenied,
        }
    }
}
