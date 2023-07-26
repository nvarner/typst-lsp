use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;
use tracing::warn;
use typst::file::FileId;
use typst::util::PathExt as TypstPathExt;
use walkdir::WalkDir;

use crate::ext::PathExt;
use crate::workspace::fs::local::LocalFs;

use super::{IdToUriError, ProjectMeta, UriToIdError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalProjectMeta {
    root_path: PathBuf,
}

impl ProjectMeta for LocalProjectMeta {
    fn uri_to_id(&self, uri: &Url) -> Result<FileId, UriToIdError> {
        let path = LocalFs::uri_to_path(uri)?;
        self.path_to_id(&path)
            .ok_or_else(|| UriToIdError::NotInProject)
    }

    fn id_to_uri(&self, id: FileId) -> Result<Url, IdToUriError> {
        let path = self.id_to_path(id);
        let uri = LocalFs::path_to_uri(&path)?;
        Ok(uri)
    }
}

// e.g. when searching via `ProjectManager`
impl LocalProjectMeta {
    pub fn new(root_path: PathBuf) -> Self {
        Self { root_path }
    }

    pub fn path(&self) -> &Path {
        &self.root_path
    }

    pub fn find_source_uris(&self) -> impl Iterator<Item = Url> {
        WalkDir::new(self.path())
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter(|file| file.path().is_typst())
            .filter_map(|file| LocalFs::path_to_uri(file.path()).ok())
    }

    /// Converts the file ID into a path
    fn id_to_path(&self, id: FileId) -> PathBuf {
        match id.package() {
            None => self
                .project_path_to_fs_path(id.path())
                .expect("file ID path is normalized, so should be in the project"),
            Some(_package) => todo!("packages not yet implemented"),
        }
    }

    /// Converts the path into a file ID if the path is in the project
    fn path_to_id(&self, path: &Path) -> Option<FileId> {
        let to_project_id = || {
            let project_path = self.fs_path_to_project_path(path)?;
            Some(FileId::new(None, &project_path))
        };

        // TODO: implement packages
        let to_package_id = || {
            warn!("packages not yet implemented");
            None
        };

        to_project_id().or_else(to_package_id)
    }

    /// Converts the path relative to the project root to a path in the filesystem if the path is in
    /// the project
    fn project_path_to_fs_path(&self, path_in_project: &Path) -> Option<PathBuf> {
        self.root_path.join_rooted(path_in_project)
    }

    /// Converts the path in the filesystem to a path relative to the project root if the path is in
    /// the project
    fn fs_path_to_project_path(&self, path: &Path) -> Option<PathBuf> {
        let project_path = path
            .strip_prefix(&self.root_path)
            .ok()?
            .push_front(Path::root());
        Some(project_path)
    }
}
