use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;
use tracing::{error, warn};
use typst::diag::{FileError, FileResult};
use typst::file::FileId;
use typst::util::PathExt as TypstPathExt;

use crate::ext::PathExt;
use crate::lsp_typst_boundary::{path_to_uri, uri_to_path};

use super::ProjectMeta;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalProjectMeta {
    root_path: PathBuf,
}

impl ProjectMeta for LocalProjectMeta {
    fn uri_to_id(&self, uri: &Url) -> FileResult<FileId> {
        let path = uri_to_path(uri)?;
        self.path_to_id(&path)
    }

    fn id_to_uri(&self, id: FileId) -> FileResult<Url> {
        let path = self.id_to_path(id)?;
        path_to_uri(&path)
    }
}

// TODO: improve return types to prevent `error!`ing on failure, since some failures are expected,
// e.g. when searching via `ProjectManager`
impl LocalProjectMeta {
    pub fn new(root_path: PathBuf) -> Self {
        Self { root_path }
    }

    pub fn path(&self) -> &Path {
        &self.root_path
    }

    fn id_to_path(&self, id: FileId) -> FileResult<PathBuf> {
        match id.package() {
            None => self.project_path_to_fs_path(id.path()),
            Some(_package) => todo!("packages not yet implemented"),
        }
    }

    fn path_to_id(&self, path: &Path) -> FileResult<FileId> {
        let to_project_id = || {
            let project_path = self.fs_path_to_project_path(path).ok()?;
            Some(FileId::new(None, &project_path))
        };

        // TODO: implement packages
        let to_package_id = || {
            warn!("packages not yet implemented");
            None
        };

        to_project_id()
            .or_else(to_package_id)
            .ok_or(FileError::Other)
    }

    fn project_path_to_fs_path(&self, path_in_project: &Path) -> FileResult<PathBuf> {
        let handle_error = || {
            error!(
                "path `{}` in project `{}` could not be made absolute",
                path_in_project.display(),
                self.root_path.display()
            );
            FileError::NotFound(path_in_project.to_owned())
        };

        self.root_path
            .join_rooted(path_in_project)
            .ok_or_else(handle_error)
    }

    fn fs_path_to_project_path(&self, path: &Path) -> FileResult<PathBuf> {
        let project_path = path
            .strip_prefix(&self.root_path)
            .map_err(|_| FileError::Other)?
            .push_front(Path::root());
        Ok(project_path)
    }
}
