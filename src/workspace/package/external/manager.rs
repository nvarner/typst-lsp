use anyhow::anyhow;
use tower_lsp::lsp_types::Url;
use tracing::{error, warn};
use typst::syntax::PackageSpec;

use crate::workspace::package::manager::{ExternalPackageError, ExternalPackageResult};
use crate::workspace::package::{FullFileId, Package};

use super::local::LocalProvider;
use super::remote_repo::RemoteRepoProvider;
use super::{ExternalPackageProvider, RepoProvider, RepoRetrievalDest};

#[derive(Debug)]
pub struct ExternalPackageManager<
    Dest: RepoRetrievalDest = LocalProvider,
    Repo: RepoProvider = Option<RemoteRepoProvider>,
> {
    providers: Vec<Box<dyn ExternalPackageProvider>>,
    cache: Option<Dest>,
    repo: Repo,
}

impl ExternalPackageManager {
    // TODO: allow configuration of these directories
    // i.e. the paths `<config>/typst/` and `<cache>/typst/` should be customizable
    pub fn new() -> Self {
        let user = dirs::config_dir()
            .map(|path| path.join("typst/packages/"))
            .map(LocalProvider::new)
            .map(Box::new)
            .map(|provider| provider as Box<dyn ExternalPackageProvider>);

        if user.is_none() {
            warn!("could not get user external package directory");
        }

        let cache = dirs::cache_dir()
            .map(|path| path.join("typst/packages/"))
            .map(LocalProvider::new);

        if cache.is_none() {
            warn!("could not get external package cache");
        }

        let providers = [
            user,
            cache
                .clone()
                .map(Box::new)
                .map(|cache| cache as Box<dyn ExternalPackageProvider>),
        ]
        .into_iter()
        .flatten()
        .collect();

        let repo = RemoteRepoProvider::new()
            .map_err(|err| error!(%err, "could not get remote repo provider for Typst packages"))
            .ok();

        Self {
            providers,
            cache,
            repo,
        }
    }
}

impl<Dest: RepoRetrievalDest, Repo: RepoProvider> ExternalPackageManager<Dest, Repo> {
    fn providers(&self) -> impl Iterator<Item = &dyn ExternalPackageProvider> {
        self.providers.iter().map(Box::as_ref)
    }

    /// Gets the package for the spec, downloading it if needed
    pub async fn package(&self, spec: &PackageSpec) -> ExternalPackageResult<Package> {
        let provider = self.providers().find_map(|provider| provider.package(spec));

        match provider {
            Some(provider) => Ok(provider),
            None => self.download_to_cache(spec).await,
        }
    }

    pub fn full_id(&self, uri: &Url) -> Option<FullFileId> {
        self.providers().find_map(|provider| provider.full_id(uri))
    }

    #[tracing::instrument]
    async fn download_to_cache(&self, spec: &PackageSpec) -> ExternalPackageResult<Package> {
        if let Some(cache) = &self.cache {
            Ok(cache.store_from(&self.repo, spec).await?)
        } else {
            Err(ExternalPackageError::Other(anyhow!(
                "nowhere to download package {spec}"
            )))
        }
    }
}
