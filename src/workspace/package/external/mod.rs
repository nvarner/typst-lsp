use std::{fmt, io};

use anyhow::anyhow;
use async_trait::async_trait;
use tokio::io::AsyncBufRead;
use tower_lsp::lsp_types::Url;
use typst::diag::{EcoString, PackageError as TypstPackageError};
use typst::syntax::PackageSpec;

use super::{FullFileId, Package};

pub mod local;
pub mod manager;
pub mod remote_repo;

/// Provides access to external packages
pub trait ExternalPackageProvider: fmt::Debug + Send + Sync {
    /// The package, if it is provided by this provider
    fn package(&self, spec: &PackageSpec) -> Option<Package>;

    /// The full ID of a file, if the file is provided by this provider
    fn full_id(&self, uri: &Url) -> Option<FullFileId>;
}

/// Provides access to package repositories. At present, this is only [https://packages.typst.org].
///
/// A package repository is a directory of packages with a mechanism for retrieving them. In
/// practice, this will probably always mean a web service from which we can download packages.
#[async_trait]
pub trait RepoProvider: fmt::Debug + Send + Sync {
    async fn retrieve_tar_gz(&self, spec: &PackageSpec)
        -> RepoResult<Box<dyn AsyncBufRead + Send>>;
}

#[async_trait]
impl<R: RepoProvider> RepoProvider for Option<R> {
    async fn retrieve_tar_gz(
        &self,
        spec: &PackageSpec,
    ) -> RepoResult<Box<dyn AsyncBufRead + Send>> {
        match self {
            Some(repo) => repo.retrieve_tar_gz(spec).await,
            None => Err(RepoError::NotFound(anyhow!(
                "no repo access to download {spec}"
            ))),
        }
    }
}

#[async_trait]
pub trait RepoRetrievalDest: fmt::Debug + Sync {
    async fn store_tar_gz(
        &self,
        spec: &PackageSpec,
        package_tar_gz: impl AsyncBufRead + Unpin + Send,
    ) -> RepoResult<Package>;

    async fn store_from<R: RepoProvider>(
        &self,
        repo: &R,
        spec: &PackageSpec,
    ) -> RepoResult<Package> {
        let tar_gz = Box::into_pin(repo.retrieve_tar_gz(spec).await?);
        self.store_tar_gz(spec, tar_gz).await
    }
}

pub type RepoResult<T> = Result<T, RepoError>;

#[derive(thiserror::Error, Debug)]
pub enum RepoError {
    #[error("cannot download packages in namespace `{0}`, only namespace `preview`")]
    InvalidNamespace(EcoString),
    #[error("could not find package")]
    NotFound(#[source] anyhow::Error),
    #[error(transparent)]
    Network(reqwest::Error),
    #[error("could not extract archive")]
    MalformedArchive(#[source] io::Error),
    #[error("error writing to local filesystem")]
    LocalFs(#[source] io::Error),
}

impl From<RepoError> for io::Error {
    fn from(err: RepoError) -> Self {
        io::Error::new(io::ErrorKind::Other, err)
    }
}

impl RepoError {
    pub fn from_archive_error(err: io::Error) -> Self {
        let err = match Self::io_as::<reqwest::Error>(err) {
            Ok(err) => {
                if err.status().map_or(false, |status| status == 404) {
                    // 404 is returned when requesting package that does not exist, but it shouldn't
                    // be used for other errors
                    return Self::NotFound(err.into());
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
