use tower_lsp::lsp_types::Url;
use typst::syntax::PackageSpec;

use super::{FullFileId, Package};

pub mod local;
pub mod manager;
pub mod repo;

/// Provides access to external packages
pub trait ExternalPackageProvider {
    /// The package, if it is provided by this provider
    fn package(&self, spec: &PackageSpec) -> Option<Package>;

    /// The full ID of a file, if the file is provided by this provider
    fn full_id(&self, uri: &Url) -> Option<FullFileId>;
}
