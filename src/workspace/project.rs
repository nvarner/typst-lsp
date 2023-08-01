use std::ops;

use tokio::sync::OwnedRwLockReadGuard;
use typst::file::FileId;

use crate::ext::FileIdExt;

use super::fs::local::UriToFsPathError;
use super::package::{FullFileId, PackageId};
use super::Workspace;

#[derive(Debug)]
pub struct Project<W = OwnedRwLockReadGuard<Workspace>>
where
    W: ops::Deref<Target = Workspace>,
{
    current: PackageId,
    workspace: W,
}

impl<W: ops::Deref<Target = Workspace>> Project<W> {
    pub fn new(current: PackageId, workspace: W) -> Self {
        Self { current, workspace }
    }

    pub fn fill_id(&self, id: FileId) -> FullFileId {
        id.fill(self.current)
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    // pub fn uri_to_id(&self, uri: &Url) -> Result<FileId, UriToIdError> {
    //     todo!()
    // }

    // fn try_uri_to_id(
    //     uri: &Url,
    //     package: &dyn Package,
    //     spec: Option<&PackageSpec>,
    // ) -> Option<Result<FileId, UriToIdError>> {
    //     match package.uri_to_package_path(uri) {
    //         Ok(path) => Some(Ok(FileId::new(spec, &path))),
    //         Err(UriToPackagePathError::NotInProject) => None,
    //         Err(err) => Some(Err(err)),
    //     }
    // }

    // pub fn id_to_uri(&self, id: FileId) -> Result<Url, IdToUriError> {
    //     let package = self.package_id(id.package());
    //     let uri = package.package_path_to_uri(id.path())?;
    //     Ok(uri)
    // }
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
