use std::collections::HashSet;
use std::path::PathBuf;

use itertools::Itertools;
use tower_lsp::lsp_types::{Url, WorkspaceFoldersChangeEvent};
use typst::diag::{FileError, FileResult};
use typst::file::FileId;

use super::local::LocalProjectMeta;
use super::ProjectMeta;

/// Determines canonical [`ProjectMeta`]s and [`FileId`]s for URIs based on the current set of
/// [`ProjectMeta`]s. That is, it will associate to any given URI the same ID and project for the
/// same underlying set of projects.
///
/// This is needed, for example, to create [`Source`](typst::Source)s in
/// [`FsProvider`](crate::workspace::fs::FsProvider)s, since they need a package and project
/// relative path to create an ID, but only have a URI.
///
/// Note also that taking just the ID may not uniquely identify a file. If there are multiple
/// non-package projects, it is possible that two have a file with the same relative path, in which
/// case their IDs will be identical.
pub struct ProjectManager {
    local: Vec<LocalProjectMeta>,
}

impl ProjectManager {
    pub fn new(root_paths: Vec<PathBuf>) -> Self {
        let local = root_paths
            .into_iter()
            .map(LocalProjectMeta::new)
            .collect_vec();
        Self { local }
    }

    pub fn handle_change_event(&mut self, event: &WorkspaceFoldersChangeEvent) {
        let paths_to_remove: HashSet<_> = event
            .removed
            .iter()
            .filter_map(|folder| folder.uri.to_file_path().ok())
            .collect();

        let local_to_add = event
            .added
            .iter()
            .filter_map(|folder| folder.uri.to_file_path().ok())
            .map(LocalProjectMeta::new);

        self.local = self
            .local
            .drain(0..)
            .filter(|local| paths_to_remove.contains(local.path()))
            .chain(local_to_add)
            .collect_vec();
    }

    pub fn find_source_uris(&self) -> impl Iterator<Item = Url> {
        self.local
            .iter()
            .map(LocalProjectMeta::find_source_uris)
            .collect_vec()
            .into_iter()
            .flatten()
    }

    pub fn uri_to_id(&self, uri: &Url) -> FileResult<FileId> {
        self.uri_to_project_and_id(uri).map(|(_, id)| id)
    }

    pub fn uri_to_project_and_id(&self, uri: &Url) -> FileResult<(Box<dyn ProjectMeta>, FileId)> {
        let candidates = self
            .local
            .iter()
            .map(|meta| (meta, meta.uri_to_id(uri).ok()))
            .filter_map(|(x, y)| y.map(|y| (x, y)));

        // Our candidates are projects containing a URI, so we expect to get a set of
        // subdirectories. The "best" is the "most specific", that is, the project that is a
        // subdirectory of the rest. This must have the longest length.
        let (best_meta, best_id) = candidates
            .max_by_key(|(local, _)| local.path().components().count())
            .ok_or_else(|| {
                uri.to_file_path()
                    .map(FileError::NotFound)
                    .unwrap_or(FileError::Other)
            })?;

        Ok((
            Box::new(best_meta.clone()) as Box<dyn ProjectMeta + Send + Sync>,
            best_id,
        ))
    }
}
