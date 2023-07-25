use std::fs;
use std::path::Path;

use tower_lsp::lsp_types::Url;
use typst::diag::{FileError, FileResult};
use typst::syntax::Source;
use typst::util::Bytes;

use crate::lsp_typst_boundary::uri_to_path;
use crate::workspace::project::manager::ProjectManager;

use super::{ReadProvider, WriteProvider};

/// Implements the Typst filesystem on the local filesystem, mapping Typst files to local files, and
/// providing conversions using [`Path`]s as an intermediate.
///
/// In this context, a "path" refers to an absolute path in the local filesystem. Paths in the Typst
/// filesystem are absolute, relative to either the project or some package. They use the same type,
/// but are meaningless when interpreted as local paths without accounting for the project or
/// package root. So, for consistency, we avoid using these Typst paths and prefer filesystem paths.
#[derive(Default)]
pub struct LocalFs {}

impl ReadProvider for LocalFs {
    type Error = FileError;

    fn read_bytes(&self, uri: &Url) -> FileResult<Bytes> {
        let path = uri_to_path(uri)?;
        Self::read_path_raw(&path).map(Bytes::from)
    }

    fn read_source(&self, uri: &Url, project_manager: &ProjectManager) -> FileResult<Source> {
        let path = uri_to_path(uri)?;

        let extension_is_typ = || path.extension().map(|ext| ext == "typ").unwrap_or(false);
        if !extension_is_typ() {
            return Err(FileError::NotSource);
        };

        let raw = Self::read_path_raw(&path)?;

        let id = project_manager.uri_to_id(uri)?;
        let text = String::from_utf8(raw).map_err(|_| FileError::InvalidUtf8)?;
        Ok(Source::new(id, text))
    }
}

impl WriteProvider for LocalFs {
    type Error = FileError;

    fn write_raw(&self, uri: &Url, data: &[u8]) -> FileResult<()> {
        let path = uri_to_path(uri)?;
        Self::write_path_raw(&path, data)
    }
}

impl LocalFs {
    /// Regular read from filesystem, returning a [`FileResult`] on failure
    pub fn read_path_raw(path: &Path) -> FileResult<Vec<u8>> {
        fs::read(path).map_err(|err| FileError::from_io(err, path))
    }

    pub fn write_path_raw(path: &Path, data: &[u8]) -> FileResult<()> {
        fs::write(path, data).map_err(|err| FileError::from_io(err, path))
    }
}
