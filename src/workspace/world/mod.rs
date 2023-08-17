use comemo::Prehashed;
use futures::Future;
use tokio::runtime;
use tower_lsp::lsp_types::Url;
use typst::diag::{EcoString, FileResult};
use typst::eval::{Bytes, Datetime, Library};
use typst::font::{Font, FontBook};
use typst::syntax::{FileId, PackageSpec, Source};
use typst::World;

use crate::workspace::fs::{FsError, FsResult};
use crate::workspace::project::Project;

use self::clock::Now;

pub mod clock;
pub mod typst_thread;

/// Short-lived struct to implement [`World`] for [`Project`]. It wraps a `Project` with a main file
/// and exists for the lifetime of a Typst invocation.
///
/// Must be created via a [`TypstThread`](self::typst_thread::TypstThread).
#[derive(Debug)]
pub struct ProjectWorld {
    project: Project,
    main: Source,
    /// Current time. Will be cached lazily for consistency throughout a compilation.
    now: Now,
    handle: runtime::Handle,
}

impl ProjectWorld {
    fn new(project: Project, main: Source, handle: runtime::Handle) -> Self {
        Self {
            project,
            main,
            now: Now::new(),
            handle,
        }
    }

    /// Write raw data to a file.
    ///
    /// This can cause cache invalidation errors if `uri` refers to a file in the cache, since the
    /// cache wouldn't know about the update. However, this is hard to fix, because we don't have
    /// `&mut self`.
    ///
    /// For example, when writing a PDF, we (effectively) have `&Workspace` after compiling via
    /// Typst, and we'd rather not lock everything just to export the PDF. However, if we allow for
    /// mutating files stored in the `Cache`, we could update a file while it is being used for a
    /// Typst compilation, which is also bad.
    pub fn write_raw(&self, uri: &Url, data: &[u8]) -> FsResult<()> {
        self.project.write_raw(uri, data)
    }

    /// Runs a `Future` in a non-async function, blocking until completion
    ///
    /// `comemo` doesn't support async, so Typst can't, so we're stuck with this for now to run
    /// async code in the `World` implementation
    pub fn block<T>(&self, fut: impl Future<Output = T>) -> T {
        self.handle.block_on(fut)
    }
}

impl World for ProjectWorld {
    #[tracing::instrument]
    fn library(&self) -> &Prehashed<Library> {
        self.project.typst_stdlib()
    }

    #[tracing::instrument]
    fn book(&self) -> &Prehashed<FontBook> {
        self.project.font_book()
    }

    #[tracing::instrument]
    fn main(&self) -> Source {
        self.main.clone()
    }

    #[tracing::instrument]
    fn source(&self, id: FileId) -> FileResult<Source> {
        self.block(self.project.read_source_by_id(id))
            .map_err(|err: FsError| err.report_and_convert(id))
    }

    #[tracing::instrument]
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.block(self.project.read_bytes_by_id(id))
            .map_err(|err: FsError| err.report_and_convert(id))
    }

    #[tracing::instrument]
    fn font(&self, id: usize) -> Option<Font> {
        self.project.font(id)
    }

    #[tracing::instrument]
    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        self.now.date_with_typst_offset(offset)
    }

    #[tracing::instrument]
    fn packages(&self) -> &[(PackageSpec, Option<EcoString>)] {
        // TODO: implement package completion
        &[]
    }
}
