use tower_lsp::lsp_types::Url;
use typst::file::PackageSpec;

use crate::workspace::package::manager::ExternalPackageResult;
use crate::workspace::package::{FullFileId, Package};

use super::local::LocalProvider;
use super::repo::RepoProvider;
use super::ExternalPackageProvider;

#[derive(Debug)]
pub struct ExternalPackageManager {
    user: LocalProvider,
    cache: LocalProvider,
    repo: RepoProvider,
}

impl ExternalPackageManager {
    // TODO: allow configuration of these directories
    pub fn new() -> Option<Self> {
        let mut user_path = dirs::config_dir()?;
        user_path.push("typst/packages/");
        let user = LocalProvider::new(user_path);

        let mut cache_path = dirs::cache_dir()?;
        cache_path.push("typst/packages/");
        let cache = LocalProvider::new(cache_path);

        let repo = RepoProvider::default();

        Some(Self { user, cache, repo })
    }

    // /// Gets the URI for the ID, downloading its package if needed
    // pub async fn uri(&self, id: FullFileId, spec: &PackageSpec) -> ExternalPackageResult<Url> {
    //     let package = self.package(spec).await?;
    //     let uri = package.path_to_uri(id.path())?;
    //     Ok(uri)
    // }

    /// Gets the package for the spec, downloading it if needed
    pub async fn package(&self, spec: &PackageSpec) -> ExternalPackageResult<Package> {
        let provider = [&self.user, &self.cache]
            .into_iter()
            .find_map(|provider| provider.package(spec));

        match provider {
            Some(provider) => Ok(provider),
            None => self.download_to_cache(spec).await,
        }
    }

    pub fn full_id(&self, uri: &Url) -> Option<FullFileId> {
        self.user.full_id(uri).or_else(|| self.cache.full_id(uri))
    }

    #[tracing::instrument]
    async fn download_to_cache(&self, spec: &PackageSpec) -> ExternalPackageResult<Package> {
        Ok(self.cache.download(spec, &self.repo).await?)
    }
}
