use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;
use tracing::error;
use typst::diag::{FileError, FileResult};
use typst::file::FileId;
use typst::util::PathExt as TypstPathExt;

use crate::ext::PathExt;

use super::ProjectConverter;

pub struct LocalProjectConverter {
    root_path: PathBuf,
}

impl ProjectConverter for LocalProjectConverter {
    fn uri_to_id(&self, uri: &Url) -> FileResult<FileId> {
        let path = self.uri_to_path(uri)?;
        self.path_to_id(&path)
    }

    fn id_to_uri(&self, id: FileId) -> FileResult<Url> {
        let path = self.id_to_path(id)?;
        self.path_to_uri(&path)
    }
}

impl LocalProjectConverter {
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

        let to_package_id = || todo!("packages not yet implemented");

        to_project_id().or_else(to_package_id).ok_or_else(|| {
            error!(path = %path.display(), "path is not in a project or package");
            FileError::NotFound(path.to_owned())
        })
    }

    fn project_path_to_fs_path(&self, path_in_project: &Path) -> FileResult<PathBuf> {
        let handle_error = || {
            error!(
                "path `{}` in project `{}` could not be made absolute",
                path_in_project.display(),
                self.project_root().display()
            );
            FileError::NotFound(path_in_project.to_owned())
        };

        self.root_path
            .join_rooted(path_in_project)
            .ok_or_else(handle_error)
    }

    fn fs_path_to_project_path(&self, path: &Path) -> FileResult<PathBuf> {
        let handle_error = |_| {
            error!(
                "path `{}` is not in the project root `{}`",
                path.display(),
                self.project_root().display()
            );
            FileError::NotFound(path.to_owned())
        };

        let project_path = path
            .strip_prefix(self.project_root())
            .map_err(handle_error)?
            .push_front(Path::root());
        Ok(project_path)
    }
}
