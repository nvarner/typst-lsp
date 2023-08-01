use core::fmt;
use std::path::{Path, PathBuf};

use internment::Intern;
use tower_lsp::lsp_types::Url;
use typst::file::{FileId, PackageSpec};

use crate::ext::{UriResult, UrlExt};

use super::fs::local::UriToFsPathError;

pub mod external;
pub mod local;
pub mod manager;

/// Represents a package that is provided. In particular, the `FsManager` should be able to access
/// files in the package via the `root` URI.
#[derive(Debug, Clone)]
pub struct Package {
    root: Url,
}

impl Package {
    pub fn new(root: Url) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Url {
        &self.root
    }

    /// Converts a path in the package to a URI
    pub fn path_to_uri(&self, path: &Path) -> UriResult<Url> {
        self.root.clone().join_rooted(path)
    }

    pub fn uri_to_path(&self, uri: &Url) -> UriResult<PathBuf> {
        self.root.make_relative_rooted(uri)
    }
}

pub trait PackageTraitOld: Send + Sync + fmt::Debug {
    fn uri_to_package_path(&self, uri: &Url) -> Result<PathBuf, UriToPackagePathError>;
    fn package_path_to_uri(&self, path: &Path) -> Result<Url, PackagePathToUriError>;
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct PackageId(Intern<PackageIdInner>);

#[derive(Debug, Hash, PartialEq, Eq)]
enum PackageIdInner {
    Current(Url),
    External(PackageSpec),
}

impl PackageId {
    pub fn new_current(root: Url) -> Self {
        Self::new(PackageIdInner::Current(root))
    }

    pub fn new_external(spec: PackageSpec) -> Self {
        Self::new(PackageIdInner::External(spec))
    }

    fn new(inner: PackageIdInner) -> Self {
        Self(Intern::new(inner))
    }

    fn inner(self) -> &'static PackageIdInner {
        self.0.as_ref()
    }

    pub fn spec(self) -> Option<&'static PackageSpec> {
        match self.inner() {
            PackageIdInner::Current(_) => None,
            PackageIdInner::External(spec) => Some(spec),
        }
    }

    // /// Converts a path in a package to a URI if the path stays in the package
    // pub fn package_path_to_uri(self, package_path: &Path) -> Option<Url> {
    //     self.root()
    //         .clone()
    //         .join_rooted(package_path)
    //         .map_err(|err| match err {
    //             UriJoinRootedError::PathEscapesRoot => (),
    //             UriJoinRootedError::UriCannotBeABase => panic!("root URI should be a base"),
    //         })
    //         .ok()
    // }
}

/// A `FullFileId` is a "more specific" [`FileId`](typst::file::FileId)
///
/// - `FileId` represents `(Option<PackageSpec>, PathBuf)`
/// - `FullFileId` represents `(PackageId, PathBuf)`
///
/// A `FileId` only makes sense in the context of a [`Project`](super::project::Project), since it
/// needs to know which is the current package, while a `FullFileId` makes sense in the more general
/// context of a [`PackageManager`](self::manager::PackageManager), since it specifies the current
/// package as needed.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct FullFileId(Intern<FullFileIdInner>);

#[derive(Debug, Hash, PartialEq, Eq)]
struct FullFileIdInner {
    package: PackageId,
    path: PathBuf,
}

impl FullFileId {
    pub fn new(package: PackageId, path: PathBuf) -> Self {
        Self(Intern::new(FullFileIdInner { package, path }))
    }

    pub fn package(self) -> PackageId {
        self.0.as_ref().package
    }

    pub fn path(self) -> &'static Path {
        &self.0.as_ref().path
    }

    pub fn spec(self) -> Option<&'static PackageSpec> {
        self.package().spec()
    }
}

impl From<FullFileId> for FileId {
    fn from(full: FullFileId) -> Self {
        Self::new(full.spec().cloned(), full.path())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum UriToPackagePathError {
    #[error("scheme of URI is not `file`")]
    SchemeIsNotFile,
    #[error("URI to path conversion error")]
    Conversion,
    #[error("URI is not in the project")]
    NotInProject,
}

impl From<UriToFsPathError> for UriToPackagePathError {
    fn from(err: UriToFsPathError) -> Self {
        match err {
            UriToFsPathError::SchemeIsNotFile => Self::SchemeIsNotFile,
            UriToFsPathError::Conversion => Self::Conversion,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum PackagePathToUriError {
    #[error("path led outside the project")]
    NotInProject,
}
