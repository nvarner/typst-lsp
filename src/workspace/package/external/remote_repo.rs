use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use async_compression::tokio::bufread::GzipDecoder;
use async_trait::async_trait;
use futures::TryStreamExt;
use reqwest::{Client, Url};
use tokio::io::{AsyncBufRead, AsyncRead};
use tokio_tar::Archive;
use tokio_util::io::StreamReader;
use typst::syntax::PackageSpec;

use super::{RepoError, RepoProvider, RepoResult};

const TYPST_REPO_BASE_URL: &str = "https://packages.typst.org/";
const PREVIEW_NAMESPACE: &str = "preview";

/// Provides access to remote package repositories
#[derive(Debug)]
pub struct RemoteRepoProvider {
    base_url: Url,
    client: Client,
}

#[async_trait]
impl RepoProvider for RemoteRepoProvider {
    async fn retrieve_tar_gz(
        &self,
        spec: &PackageSpec,
    ) -> RepoResult<Box<dyn AsyncBufRead + Send>> {
        // We don't know how packages will change once they leave preview, so restrict downloads to
        // preview for now
        if spec.namespace != PREVIEW_NAMESPACE {
            return Err(RepoError::InvalidNamespace(spec.namespace.clone()));
        }

        let url = self.url(spec);
        let downloaded = self.download_raw(url).await?;
        Ok(Box::new(downloaded))
    }

    async fn retrieve_index(&self) -> RepoResult<Box<dyn AsyncBufRead + Send>> {
        // typicially, it is https://packages.typst.org/preview/index.json
        let url = self.index_url(PREVIEW_NAMESPACE);
        let downloaded = self.download_raw(url).await?;
        Ok(Box::new(downloaded))
    }
}

impl RemoteRepoProvider {
    pub fn new() -> anyhow::Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .context("couldn't read system configuration for HTTP client")?;

        Ok(Self {
            base_url: Url::parse(TYPST_REPO_BASE_URL).unwrap(),
            client,
        })
    }

    #[tracing::instrument(skip(path), fields(path = %path.as_ref().display()))]
    pub async fn download_to(&self, spec: &PackageSpec, path: impl AsRef<Path>) -> RepoResult<()> {
        // We don't know how packages will change once they leave preview, so restrict downloads to
        // preview for now
        if spec.namespace != PREVIEW_NAMESPACE {
            return Err(RepoError::InvalidNamespace(spec.namespace.clone()));
        }

        let url = self.url(spec);
        let downloaded = self.download_raw(url).await?;
        let decompressed = self.decompress(downloaded);
        self.unpack_to(decompressed, path).await?;
        Ok(())
    }

    fn url(&self, spec: &PackageSpec) -> Url {
        let path = format!("{}/{}-{}.tar.gz", spec.namespace, spec.name, spec.version);
        self.base_url.join(&path).expect("should be a valid URL")
    }

    fn index_url(&self, namespace: &str) -> Url {
        let path = format!("{namespace}/index.json");
        self.base_url.join(&path).expect("should be a valid URL")
    }

    async fn download_raw(&self, url: Url) -> RepoResult<impl AsyncBufRead + Unpin> {
        let stream = self
            .client
            .get(url)
            .send()
            .await
            .map_err(RepoError::Network)?
            .bytes_stream()
            .map_err(RepoError::Network);
        Ok(StreamReader::new(stream))
    }

    fn decompress(&self, downloaded: impl AsyncBufRead + Unpin) -> impl AsyncRead + Unpin {
        GzipDecoder::new(downloaded)
    }

    async fn unpack_to(
        &self,
        decompressed: impl AsyncRead + Unpin,
        path: impl AsRef<Path>,
    ) -> RepoResult<()> {
        Archive::new(decompressed)
            .unpack(path.as_ref())
            .await
            .map_err(RepoError::from_archive_error)
    }
}

impl Default for RemoteRepoProvider {
    fn default() -> Self {
        Self {
            base_url: Url::parse(TYPST_REPO_BASE_URL).unwrap(),
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(5))
                .build()
                .expect("couldn't read system configuration for HTTP client"),
        }
    }
}

#[cfg(test)]
mod test {
    use futures::future::try_join_all;
    use temp_dir::TempDir;
    use tokio::fs;

    use super::*;

    #[tokio::test]
    async fn full_download() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path();

        let spec = "@preview/example:0.1.0".parse().unwrap();

        let provider = RemoteRepoProvider::new().unwrap();
        provider.download_to(&spec, target).await?;

        let all_exist = try_join_all(vec![
            fs::try_exists(target.join("typst.toml")),
            fs::try_exists(target.join("lib.typ")),
            fs::try_exists(target.join("LICENSE")),
            fs::try_exists(target.join("README.md")),
            fs::try_exists(target.join("util/")),
            fs::try_exists(target.join("util/math.typ")),
        ])
        .await?
        .into_iter()
        .all(|x| x);

        assert!(all_exist);

        Ok(())
    }
}
