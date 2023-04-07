use tower_lsp::lsp_types::Url;

use crate::lsp_typst_boundary::{lsp_to_typst, TypstSource};

use super::source_manager::SourceId;

/// Typst source file
#[derive(Debug)]
pub struct Source {
    inner: TypstSource,
}

impl Source {
    pub fn new(id: SourceId, uri: &Url, text: String) -> Self {
        let typst_path = lsp_to_typst::uri_to_path(uri);

        Self {
            inner: TypstSource::new(id.into(), &typst_path, text),
        }
    }
}

impl AsRef<TypstSource> for Source {
    fn as_ref(&self) -> &TypstSource {
        &self.inner
    }
}
