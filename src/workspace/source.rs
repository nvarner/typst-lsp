use tower_lsp::lsp_types::Url;
use typst::syntax::SourceId;

use crate::lsp_typst_boundary::{lsp_to_typst, LspRange, TypstSource};

/// Typst source file
#[derive(Debug)]
pub struct Source {
    inner: TypstSource,
}

impl Source {
    pub fn new(id: SourceId, uri: &Url, text: String) -> anyhow::Result<Self> {
        let typst_path = lsp_to_typst::uri_to_path(uri)?;

        Ok(Self {
            inner: TypstSource::new(id, &typst_path, text),
        })
    }

    pub fn new_detached() -> Self {
        Self {
            inner: TypstSource::detached(""),
        }
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
