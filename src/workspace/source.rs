use crate::lsp_typst_boundary::TypstSource;

/// Typst source file
#[derive(Debug)]
pub struct Source {
    inner: TypstSource,
}

impl AsRef<TypstSource> for Source {
    fn as_ref(&self) -> &TypstSource {
        &self.inner
    }
}
