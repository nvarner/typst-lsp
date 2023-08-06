use core::fmt;

use comemo::Prehashed;
use tokio::sync::OwnedRwLockReadGuard;
use tower_lsp::lsp_types::Url;
use typst::eval::Library;
use typst::file::FileId;
use typst::font::{Font, FontBook};
use typst::syntax::Source;
use typst::util::Bytes;

use crate::ext::FileIdExt;

use super::fs::local::UriToFsPathError;
use super::fs::FsResult;
use super::package::{FullFileId, PackageId};
use super::Workspace;

pub struct Project {
    current: PackageId,
    workspace: OwnedRwLockReadGuard<Workspace>,
}

impl Project {
    pub fn new(current: PackageId, workspace: OwnedRwLockReadGuard<Workspace>) -> Self {
        Self { current, workspace }
    }

    fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub fn typst_stdlib(&self) -> &Prehashed<Library> {
        &self.workspace().typst_stdlib
    }

    pub fn font_book(&self) -> &Prehashed<FontBook> {
        self.workspace().font_manager().book()
    }

    pub fn font(&self, id: usize) -> Option<Font> {
        self.workspace().font_manager().font(id)
    }

    pub fn fill_id(&self, id: FileId) -> FullFileId {
        id.fill(self.current)
    }

    pub async fn full_id_to_uri(&self, full_id: FullFileId) -> FsResult<Url> {
        self.workspace().uri(full_id).await
    }

    pub fn read_source_by_uri(&self, uri: &Url) -> FsResult<Source> {
        self.workspace().read_source(uri)
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
        self.workspace().write_raw(uri, data)
    }

    pub async fn read_source_by_id(&self, id: FileId) -> FsResult<Source> {
        let full_id = self.fill_id(id);
        let uri = self.full_id_to_uri(full_id).await?;
        let source = self.read_source_by_uri(&uri)?;
        Ok(source)
    }

    pub async fn read_bytes_by_id(&self, id: FileId) -> FsResult<Bytes> {
        let full_id = self.fill_id(id);
        let uri = self.full_id_to_uri(full_id).await?;
        let bytes = self.workspace().read_bytes(&uri)?;
        Ok(bytes)
    }
}

impl fmt::Debug for Project {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Project")
            .field("current", &self.current)
            .field("workspace", &"...")
            .finish()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum UriToIdError {
    #[error("cannot convert to ID since URI is not in the described project")]
    NotInProject,
    #[error(transparent)]
    Other(anyhow::Error),
}

impl From<UriToFsPathError> for UriToIdError {
    fn from(err: UriToFsPathError) -> Self {
        match err {
            UriToFsPathError::SchemeIsNotFile => Self::NotInProject,
            UriToFsPathError::Conversion => Self::Other(err.into()),
        }
    }
}
