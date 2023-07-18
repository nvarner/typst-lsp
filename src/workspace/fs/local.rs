use std::fs;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;
use tracing::error;
use typst::diag::{FileError, FileResult};
use typst::file::FileId;
use typst::util::PathExt as TypstPathExt;

use crate::ext::PathExt;

use super::TypstFs;

/// Implements the Typst filesystem on the local filesystem, mapping Typst files to local files, and
/// providing conversions using [`Path`]s as an intermediate.
///
/// In this context, a "path" refers to an absolute path in the local filesystem. Paths in the Typst
/// filesystem are absolute, relative to either the project or some package. They use the same type,
/// but are meaningless when interpreted as local paths without accounting for the project or
/// package root. So, for consistency, we avoid using these Typst paths and prefer filesystem paths.
pub struct LocalFs {
    pub(crate) project_root: PathBuf,
}

impl TypstFs for LocalFs {
    fn read_raw(&self, id: FileId) -> FileResult<Vec<u8>> {
        let path = self.id_to_path(id)?;
        Self::read_path_raw(&path)
    }

    fn uri_to_id(&self, uri: &Url) -> FileResult<FileId> {
        let path = self.uri_to_path(uri)?;
        self.path_to_id(&path)
    }

    fn id_to_uri(&self, id: FileId) -> FileResult<Url> {
        let path = self.id_to_path(id)?;
        self.path_to_uri(&path)
    }
}

impl LocalFs {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Regular read from filesystem, returning a [`FileResult`] on failure
    pub fn read_path_raw(path: &Path) -> FileResult<Vec<u8>> {
        fs::read(&path).map_err(|err| FileError::from_io(err, path))
    }

    fn project_root(&self) -> &Path {
        &self.project_root
    }
}

// Conversions between file ID/path, URI/path, and project path/fs path
impl LocalFs {
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

    fn uri_to_path(&self, uri: &Url) -> FileResult<PathBuf> {
        let is_local = |uri: &Url| uri.scheme() == "file";
        let handle_not_local = || format!("URI scheme `{}` is not `file`", uri.scheme());
        let verify_local = |uri| is_local(uri).then_some(uri).ok_or_else(handle_not_local);

        let handle_make_local_error = |()| "could not convert URI to path".to_owned();
        let make_local = |uri: &Url| uri.to_file_path().map_err(handle_make_local_error);

        let handle_error = |err| {
            error!(%uri, message = err);
            FileError::Other
        };

        verify_local(uri).and_then(make_local).map_err(handle_error)
    }

    fn path_to_uri(&self, path: &Path) -> FileResult<Url> {
        let handle_error = |()| {
            error!(path = %path.display(), "could not convert path to URI");
            FileError::NotFound(path.to_owned())
        };

        Url::from_file_path(path).map_err(handle_error)
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

        self.project_root()
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
