use anyhow::anyhow;
use tokio::io::AsyncReadExt;
use tokio::sync::OnceCell;
use tower_lsp::lsp_types::Url;
use tracing::{info, warn};
use typst::diag::EcoString;
use typst::syntax::package::{PackageSpec, PackageVersion};

use crate::workspace::package::manager::{ExternalPackageError, ExternalPackageResult};
use crate::workspace::package::{FullFileId, Package};

use super::local::LocalProvider;
use super::{ExternalPackageProvider, RepoProvider, RepoRetrievalDest};

#[cfg(feature = "remote-packages")]
type DefaultRepoProvider = Option<super::remote_repo::RemoteRepoProvider>;
#[cfg(not(feature = "remote-packages"))]
type DefaultRepoProvider = ();

#[cfg(feature = "remote-packages")]
fn get_default_repo_provider() -> DefaultRepoProvider {
    super::remote_repo::RemoteRepoProvider::new()
        .map_err(|err| warn!(%err, "could not get repo provider for Typst packages"))
        .ok()
}
#[cfg(not(feature = "remote-packages"))]
fn get_default_repo_provider() -> DefaultRepoProvider {}

#[derive(Debug)]
pub struct ExternalPackageManager<
    Dest: RepoRetrievalDest = LocalProvider,
    Repo: RepoProvider = DefaultRepoProvider,
> {
    providers: Vec<Box<dyn ExternalPackageProvider>>,
    cache: Option<Dest>,
    repo: Repo,
    packages: OnceCell<Vec<(PackageSpec, Option<EcoString>)>>,
}

impl ExternalPackageManager {
    // TODO: allow configuration of these directories
    // i.e. the paths `<config>/typst/` and `<cache>/typst/` should be customizable
    #[tracing::instrument]
    pub fn new() -> Self {
        let user = dirs::data_dir()
            .map(|path| path.join("typst/packages/"))
            .map(LocalProvider::new)
            .map(Box::new)
            .map(|provider| provider as Box<dyn ExternalPackageProvider>);

        if let Some(user) = &user {
            info!(?user, "got user external package directory");
        } else {
            warn!("could not get user external package directory");
        }

        let cache = dirs::cache_dir()
            .map(|path| path.join("typst/packages/"))
            .map(LocalProvider::new);

        if let Some(cache) = &cache {
            info!(?cache, "got external package cache");
        } else {
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

        Self {
            providers,
            cache,
            repo: get_default_repo_provider(),
            packages: OnceCell::default(),
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

    async fn packages_inner(&self) -> ExternalPackageResult<Vec<(PackageSpec, Option<EcoString>)>> {
        let mut buf = vec![];
        let mut index = Box::into_pin(self.repo.retrieve_index().await?);
        index.read_to_end(&mut buf).await.map_err(|err| {
            ExternalPackageError::Other(anyhow!("could not read index from repo provider: {err}"))
        })?;

        #[derive(serde::Deserialize)]
        struct RemotePackageIndex {
            name: EcoString,
            version: PackageVersion,
            description: Option<EcoString>,
        }

        Ok(serde_json::from_slice::<Vec<RemotePackageIndex>>(&buf)
            .map_err(|err| ExternalPackageError::Other(anyhow!(err)))?
            .into_iter()
            .map(
                |RemotePackageIndex {
                     name,
                     version,
                     description,
                 }| {
                    (
                        PackageSpec {
                            namespace: "preview".into(),
                            name,
                            version,
                        },
                        description,
                    )
                },
            )
            .collect::<Vec<_>>())
    }

    #[tracing::instrument]
    pub async fn packages(&self) -> &[(PackageSpec, Option<EcoString>)] {
        self.packages
            .get_or_init(|| async {
                match self.packages_inner().await {
                    Ok(index) => index,
                    Err(err) => {
                        warn!(%err, "could not get packages from repo provider");
                        vec![]
                    }
                }
            })
            .await
            .as_slice()
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;
    use std::str::FromStr;

    use tokio::fs;

    use crate::workspace::fs::local::LocalFs;

    use super::*;

    #[tokio::test]
    async fn local_package() {
        let example_local_package = ExampleLocalPackage::set_up().await;
        let spec = example_local_package.spec();
        let external_package_manager = ExternalPackageManager::new();

        let package = external_package_manager.package(&spec).await.unwrap();

        assert_eq!(example_local_package.package(), package);
    }

    pub struct ExampleLocalPackage {
        root: PathBuf,
    }

    impl ExampleLocalPackage {
        pub async fn set_up() -> Self {
            // The testing package is based on @preview/example:0.1.0
            // https://github.com/typst/packages/tree/main/packages/preview/example/0.1.0

            let package_root_path = Self::root();
            fs::create_dir_all(&package_root_path).await.unwrap();

            // Modified name vs original
            let manifest = r#"[package]
name = "typst-lsp-testing-this-may-be-deleted"
version = "0.1.0"
entrypoint = "lib.typ"
authors = ["The Typst Project Developers"]
license = "Unlicense"
description = "An example package."
"#;
            fs::write(package_root_path.join("typst.toml"), manifest)
                .await
                .unwrap();

            // Modified from the original to remove import for minimal test setup
            let lib_typ = r#"// A package can contain includable markup just like other files.
This is an *example!*

// Paths are package local and absolute paths refer to the package root.
"#;
            fs::write(package_root_path.join("lib.typ"), lib_typ)
                .await
                .unwrap();

            Self {
                root: package_root_path,
            }
        }

        pub fn spec(&self) -> PackageSpec {
            PackageSpec::from_str("@local/typst-lsp-testing-this-may-be-deleted:0.1.0").unwrap()
        }

        pub fn package(&self) -> Package {
            let package_root = Self::root();
            Package::new(LocalFs::path_to_uri(package_root).unwrap())
        }

        fn root() -> PathBuf {
            let local_packages_root = dirs::data_dir().unwrap();
            local_packages_root
                .join("typst/packages/local/typst-lsp-testing-this-may-be-deleted/0.1.0")
        }
    }

    impl Drop for ExampleLocalPackage {
        fn drop(&mut self) {
            // No async drop, so delete with sync operations
            std::fs::remove_dir_all(&self.root).unwrap();
        }
    }
}
