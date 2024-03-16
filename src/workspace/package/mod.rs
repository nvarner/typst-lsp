use core::fmt;
use std::ffi::OsStr;

use internment::Intern;
use tower_lsp::lsp_types::Url;
use typst::syntax::package::PackageSpec;
use typst::syntax::{FileId, VirtualPath};

use crate::ext::{UriResult, UrlExt, VirtualPathExt};

pub mod external;
pub mod manager;

/// Represents a package that is provided. In particular, the `FsManager` should be able to access
/// files in the package via the `root` URI.
#[derive(Clone, PartialEq, Eq)]
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
    pub fn vpath_to_uri(&self, vpath: &VirtualPath) -> UriResult<Url> {
        self.root.clone().join_rooted(vpath)
    }

    pub fn uri_to_vpath(&self, uri: &Url) -> UriResult<VirtualPath> {
        self.root.make_relative_rooted(uri)
    }
}

impl fmt::Debug for Package {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Package")
            .field("root", &self.root.as_str())
            .finish()
    }
}

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub struct PackageId(Intern<PackageIdInner>);

impl fmt::Debug for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0.as_ref() {
            PackageIdInner::Current(uri) => f
                .debug_tuple("PackageId::Current")
                .field(&uri.as_str())
                .finish(),
            PackageIdInner::External(spec) => {
                f.debug_tuple("PackageId::External").field(spec).finish()
            }
        }
    }
}

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
#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub struct FullFileId(Intern<FullFileIdInner>);

impl fmt::Debug for FullFileId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("FullFileId")
            .field(&self.0.package)
            .field(&self.0.vpath)
            .finish()
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct FullFileIdInner {
    package: PackageId,
    vpath: VirtualPath,
}

impl FullFileId {
    pub fn new(package: PackageId, vpath: VirtualPath) -> Self {
        Self(Intern::new(FullFileIdInner { package, vpath }))
    }

    pub fn package(self) -> PackageId {
        self.0.as_ref().package
    }

    pub fn vpath(self) -> &'static VirtualPath {
        &self.0.as_ref().vpath
    }

    pub fn spec(self) -> Option<&'static PackageSpec> {
        self.package().spec()
    }

    pub fn with_extension(self, extension: impl AsRef<OsStr>) -> Self {
        Self(Intern::new(FullFileIdInner {
            package: self.package(),
            vpath: self.vpath().with_extension(extension),
        }))
    }
}

impl From<FullFileId> for FileId {
    fn from(full: FullFileId) -> Self {
        Self::new(full.spec().cloned(), full.vpath().clone())
    }
}
