use std::io;
use std::path::Path;
use std::time::Duration;

use async_compression::tokio::bufread::GzipDecoder;
use futures::TryStreamExt;
use reqwest::{Client, Url};
use tokio::io::{AsyncBufRead, AsyncRead};
use tokio_tar::Archive;
use tokio_util::io::StreamReader;
use tracing::error;
use typst::diag::{EcoString, PackageError as TypstPackageError};
use typst::syntax::PackageSpec;

const TYPST_REPO_BASE_URL: &str = "https://packages.typst.org/";
const PREVIEW_NAMESPACE: &str = "preview";

/// Provides access to a package repository. At present, this is only [https://packages.typst.org].
#[derive(Debug)]
pub struct RepoProvider {
    base_url: Url,
    client: Client,
}

impl RepoProvider {
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

    async fn download_raw(&self, url: Url) -> RepoResult<impl AsyncBufRead + Unpin> {
        let stream = self
            .client
            .get(url)
            .send()
            .await
            .map_err(RepoError::Network)?
            .bytes_stream()
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err));
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

impl Default for RepoProvider {
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

pub type RepoResult<T> = Result<T, RepoError>;

#[derive(thiserror::Error, Debug)]
pub enum RepoError {
    #[error("cannot download packages in namespace `{0}`, only namespace `preview`")]
    InvalidNamespace(EcoString),
    #[error("could not find package")]
    NotFound(#[source] reqwest::Error),
    #[error(transparent)]
    Network(reqwest::Error),
    #[error("could not extract archive")]
    MalformedArchive(#[source] io::Error),
    #[error("error writing to local filesystem")]
    LocalFs(#[source] io::Error),
}

impl RepoError {
    pub fn from_archive_error(err: io::Error) -> Self {
        let err = match Self::io_as::<reqwest::Error>(err) {
            Ok(err) => {
                if err.status().map_or(false, |status| status == 404) {
                    // 404 is returned when requesting package that does not exist, but it shouldn't
                    // be used for other errors
                    return Self::NotFound(err);
                } else {
                    return Self::Network(err);
                }
            }
            Err(err) => err,
        };

        if Self::guess_is_tar_error(&err) {
            Self::MalformedArchive(err)
        } else {
            Self::LocalFs(err)
        }
    }

    /// The `tar` crate and its descendants, including `tokio-tar`, hide their errors behind
    /// [`io::Error`] and do not export their error type, so there is no direct way to tell if an
    /// error is due to a bad archive or from writing files. So, we look at the error message and
    /// try to guess if it came from a bad archive.
    fn guess_is_tar_error(err: &io::Error) -> bool {
        let message = err.to_string();
        message.contains("entries") || message.contains("archive") || message.contains("sparse")
    }

    fn io_as<T: std::error::Error + 'static>(err: io::Error) -> Result<T, io::Error> {
        // Until `io::Error::downcast` is stabilized, we need to do it this way; `into_inner` takes
        // ownership of `err` and returns `None` if it's not the requested type, so we lose `err`.
        if err.get_ref().is_some_and(|err| err.is::<reqwest::Error>()) {
            let req_err = err
                .into_inner()
                .and_then(|err| err.downcast().ok())
                .unwrap();
            Ok(*req_err)
        } else {
            Err(err)
        }
    }

    pub fn convert(self, spec: &PackageSpec) -> TypstPackageError {
        match self {
            Self::InvalidNamespace(_) | Self::NotFound(_) => {
                TypstPackageError::NotFound(spec.clone())
            }
            Self::Network(_) => TypstPackageError::NetworkFailed,
            Self::MalformedArchive(_) => TypstPackageError::MalformedArchive,
            Self::LocalFs(_) => TypstPackageError::Other,
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

        let provider = RepoProvider::default();
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
