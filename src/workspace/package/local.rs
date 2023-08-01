use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;
use typst::util::PathExt as TypstPathExt;
use walkdir::WalkDir;

use crate::ext::PathExt;
use crate::workspace::fs::local::LocalFs;

use super::{PackagePathToUriError, PackageTraitOld, UriToPackagePathError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPackage {
    root_path: PathBuf,
}

impl PackageTraitOld for LocalPackage {
    fn uri_to_package_path(&self, uri: &Url) -> Result<PathBuf, UriToPackagePathError> {
        let fs_path = LocalFs::uri_to_path(uri)?;
        self.fs_path_to_package_path(&fs_path)
            .ok_or_else(|| UriToPackagePathError::NotInProject)
    }

    fn package_path_to_uri(&self, path: &Path) -> Result<Url, PackagePathToUriError> {
        let fs_path = self
            .package_path_to_fs_path(path)
            .ok_or_else(|| PackagePathToUriError::NotInProject)?;
        let uri = LocalFs::path_to_uri(&fs_path).expect("path should be absolute");
        Ok(uri)
    }
}

impl LocalPackage {
    pub fn new(root_path: PathBuf) -> Self {
        Self { root_path }
    }

    pub fn path(&self) -> &Path {
        &self.root_path
    }

    pub fn find_source_uris(&self) -> impl Iterator<Item = Url> {
        WalkDir::new(&self.root_path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter(|file| file.path().is_typst())
            .filter_map(|file| LocalFs::path_to_uri(file.path()).ok())
    }

    /// Converts the path relative to the package root to a path in the filesystem if the path is in
    /// the package
    fn package_path_to_fs_path(&self, package_path: &Path) -> Option<PathBuf> {
        self.root_path.join_rooted(package_path)
    }

    /// Converts the path in the filesystem to a path relative to the package root if the path is in
    /// the package
    fn fs_path_to_package_path(&self, path: &Path) -> Option<PathBuf> {
        let package_path = path
            .strip_prefix(&self.root_path)
            .ok()?
            .push_front(Path::root());
        Some(package_path)
    }
}
