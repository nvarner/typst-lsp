use std::collections::HashMap;

use anyhow::anyhow;
use itertools::Itertools;
use tower_lsp::lsp_types::{Url, WorkspaceFoldersChangeEvent};
use tracing::{error, info, trace, warn};
use typst::diag::{EcoString, FileError, PackageError as TypstPackageError};
use typst::syntax::{FileId, package::PackageSpec};

use crate::ext::{UriError, UrlExt};
use crate::workspace::fs::{FsError, FsResult};
use crate::workspace::package::external::manager::ExternalPackageManager;

use super::external::RepoError;
use super::{FullFileId, Package, PackageId, PackageIdInner};

/// Determines canonical [`Package`]s and [`FileId`]s for URIs based on the current set of
/// [`Package`]s. That is, it will associate to any given URI the same ID and project for the
/// same underlying set of projects.
///
/// This is needed, for example, to create [`Source`](typst::Source)s in
/// [`ReadProvider`](crate::workspace::fs::ReadProvider)s, since they need a package and project
/// relative path to create an ID, but only have a URI.
///
/// Note also that taking just the ID may not uniquely identify a file. If there are multiple
/// non-package projects, it is possible that two have a file with the same relative path, in which
/// case their IDs will be identical.
#[derive(Debug)]
pub struct PackageManager {
    current: HashMap<Url, Package>,
    external: ExternalPackageManager,
}

impl PackageManager {
    #[tracing::instrument]
    pub fn new(root_uris: Vec<Url>, external: ExternalPackageManager) -> Self {
        let current = root_uris
            .into_iter()
            .map(|uri| (uri.clone(), Package::new(uri)))
            .collect();

        info!(?current, ?external, "initialized package manager");

        Self { current, external }
    }

    pub async fn package(&self, id: PackageId) -> PackageResult<Package> {
        let package = match id.inner() {
            PackageIdInner::Current(uri) => self
                .current
                .get(uri)
                .cloned()
                .or_else(|| {
                    trace!(
                        ?uri,
                        "Take a single file's parent directory as the package root!"
                    );

                    Some(Package::new(uri.clone()))
                })
                .ok_or(CurrentPackageError::NotFound)?,
            PackageIdInner::External(spec) => self.external_package(spec).await?,
        };
        Ok(package)
    }

    async fn external_package(&self, spec: &PackageSpec) -> ExternalPackageResult<Package> {
        self.external.package(spec).await
    }

    pub fn full_id(&self, uri: &Url) -> FsResult<FullFileId> {
        self.external
            .full_id(uri)
            .or_else(|| self.current_full_id(uri))
            .or_else(|| self.current_single_file_full_id(uri))
            .ok_or_else(|| FsError::NotProvided(anyhow!("could not find provider for URI")))
    }

    fn current_full_id(&self, uri: &Url) -> Option<FullFileId> {
        let candidates = self
            .current
            .iter()
            .filter_map(|(root, package)| Some((root, package.uri_to_vpath(uri).ok()?)))
            .inspect(|(package_root, path)| trace!(%package_root, ?path, %uri, "considering candidate for full id"));

        // Our candidates are projects containing a URI, so we expect to get a set of
        // subdirectories. The "best" is the "most specific", that is, the project that is a
        // subdirectory of the rest. This should have the longest length.
        let (best_package_root, best_path) =
            candidates.max_by_key(|(_, path)| path.as_rootless_path().components().count())?;

        let package_id = PackageId::new_current(best_package_root.clone());
        let full_file_id = FullFileId::new(package_id, best_path);

        trace!(?full_file_id, "chose full id!");

        Some(full_file_id)
    }

    fn current_single_file_full_id(&self, uri: &Url) -> Option<FullFileId> {
        // Take uri's parent directory as the package root…
        let mut root = uri.clone();
        root.path_segments_mut().ok()?.pop();

        // … and its filename as the path
        let path = root.make_relative_rooted(uri).ok()?;
        // This is equivalent to `Package::new(root.clone()).uri_to_path(uri).ok()?`,
        // but doesn't clone `root`.

        let package_id = PackageId::new_current(root);
        let full_file_id = FullFileId::new(package_id, path);

        trace!(?full_file_id, "chose a single file full id!");

        Some(full_file_id)
    }

    #[tracing::instrument]
    pub fn handle_change_event(&mut self, event: &WorkspaceFoldersChangeEvent) {
        let removed = event.removed.iter().map(|folder| &folder.uri).collect_vec();

        let added = event
            .added
            .iter()
            .map(|folder| (folder.uri.clone(), Package::new(folder.uri.clone())));

        self.current.retain(|uri, _| !removed.contains(&uri));
        self.current.extend(added);

        info!(current = ?self.current, "updated current packages");
    }

    pub fn current(&self) -> impl Iterator<Item = &Package> {
        self.current.values()
    }

    pub async fn packages(&self) -> &[(PackageSpec, Option<EcoString>)] {
        self.external.packages().await
    }
}

pub type PackageResult<T> = Result<T, PackageError>;

#[derive(thiserror::Error, Debug)]
pub enum PackageError {
    #[error(transparent)]
    Current(#[from] CurrentPackageError),
    #[error(transparent)]
    External(#[from] ExternalPackageError),
}

impl PackageError {
    pub fn convert(self, id: FileId) -> FileError {
        match self {
            Self::Current(err) => err.convert(id),
            Self::External(err) => err.convert(id),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CurrentPackageError {
    #[error("could not find current package")]
    NotFound,
}

impl CurrentPackageError {
    pub fn convert(self, id: FileId) -> FileError {
        match self {
            Self::NotFound => FileError::NotFound(id.vpath().as_rootless_path().to_owned()),
        }
    }
}

pub type ExternalPackageResult<T> = Result<T, ExternalPackageError>;

#[derive(thiserror::Error, Debug)]
pub enum ExternalPackageError {
    #[error("could not get package from repository")]
    Repo(#[from] RepoError),
    #[error("the path was invalid inside the package")]
    InvalidPath(#[from] UriError),
    #[error(transparent)]
    Other(anyhow::Error),
}

impl ExternalPackageError {
    pub fn convert(self, id: FileId) -> FileError {
        let Some(spec) = id.package() else {
            error!(?id, "cannot get spec to report `PackageError`");
            return FileError::Package(TypstPackageError::Other(Some(self.to_string().into())));
        };

        match self {
            Self::Repo(err) => FileError::Package(err.convert(spec)),
            Self::InvalidPath(_) | Self::Other(_) => {
                FileError::Other(Some(self.to_string().into()))
            }
        }
    }
}
