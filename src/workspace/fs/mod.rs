use tower_lsp::lsp_types::Url;
use typst::syntax::Source;
use typst::util::Bytes;

pub mod local;
pub mod lsp;
pub mod manager;

/// Implements the Typst filesystem for a single workspace.
///
/// Implementations provide access to project and package files, downloading packages as needed to
/// ensure their availability.
pub trait FsProvider {
    type Error;

    fn read_bytes(&self, uri: &Url) -> Result<Bytes, Self::Error>;
    fn read_source(&self, uri: &Url) -> Result<Source, Self::Error>;
}
