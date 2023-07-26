use std::collections::HashSet;

use tower_lsp::lsp_types::Url;
use typst::syntax::Source;
use typst::util::Bytes;

use super::project::manager::ProjectManager;

pub mod cache;
pub mod local;
pub mod lsp;
pub mod manager;

/// Read access to the Typst filesystem for a single workspace
pub trait ReadProvider {
    type Error;

    fn read_bytes(&self, uri: &Url) -> Result<Bytes, Self::Error>;
    fn read_source(
        &self,
        uri: &Url,
        project_manager: &ProjectManager,
    ) -> Result<Source, Self::Error>;
}

/// Write access to the Typst filesystem for a single workspace
pub trait WriteProvider {
    type Error;

    fn write_raw(&self, uri: &Url, data: &[u8]) -> Result<(), Self::Error>;
}

/// Remembers URIs if available sources
pub trait KnownUriProvider {
    fn known_uris(&self) -> HashSet<Url>;
}
