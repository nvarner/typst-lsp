use std::{fs, io};

use tower_lsp::lsp_types::Url;
use typst::diag::{FileError, FileResult};
use typst::file::FileId;

use crate::lsp_typst_boundary::{lsp_to_typst, LspRange, TypstSource};

/// Typst source file
#[derive(Debug, Clone)]
pub struct Source {
    inner: TypstSource,
}

impl Source {
    pub fn new(id: FileId, uri: &Url, text: String) -> anyhow::Result<Self> {
        let typst_path = lsp_to_typst::uri_to_path(uri)?;

        Ok(Self {
            inner: TypstSource::new(id, text),
        })
    }

    pub fn new_detached() -> Self {
        Self {
            inner: TypstSource::detached(""),
        }
    }

    pub fn read_from_file(id: SourceId, uri: &Url) -> FileResult<Self> {
        let path = lsp_to_typst::uri_to_path(uri).map_err(|_| FileError::Other)?;
        let text = fs::read_to_string(&path).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => FileError::NotFound(path),
            io::ErrorKind::PermissionDenied => FileError::AccessDenied,
            _ => FileError::Other,
        })?;
        Self::new(id, uri, text).map_err(|_| FileError::Other)
    }

    pub fn edit(&mut self, replace: &LspRange, with: &str) {
        let typst_replace = lsp_to_typst::range(replace, &self.inner);
        self.inner.edit(typst_replace, with);
    }

    pub fn replace(&mut self, text: String) {
        self.inner.replace(text);
    }
}

impl AsRef<TypstSource> for Source {
    fn as_ref(&self) -> &TypstSource {
        &self.inner
    }
}
