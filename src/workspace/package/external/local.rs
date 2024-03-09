use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use async_compression::tokio::bufread::GzipDecoder;
use async_trait::async_trait;
use tokio::io::{AsyncBufRead, AsyncRead};
use tokio_tar::Archive;
use tower_lsp::lsp_types::Url;
use typst::syntax::package::PackageSpec;
use typst::syntax::VirtualPath;

use crate::workspace::fs::local::LocalFs;
use crate::workspace::package::{FullFileId, Package, PackageId};

use super::{ExternalPackageProvider, RepoError, RepoResult, RepoRetrievalDest};

// TODO: cache packages so we don't need to do IO to check if a package is provided
#[derive(Debug, Clone)]
pub struct LocalProvider {
    root: PathBuf,
}

impl ExternalPackageProvider for LocalProvider {
    fn package(&self, spec: &PackageSpec) -> Option<Package> {
        let path = self.fs_path(spec);
        let manifest = path.join("typst.toml");

        if manifest.is_file() {
            let uri = LocalFs::path_to_uri(path).expect("should be absolute");
            Some(Package::new(uri))
        } else {
            None
        }
    }

    fn full_id(&self, uri: &Url) -> Option<FullFileId> {
        let path = LocalFs::uri_to_path(uri).ok()?;
        let relative_path = path.strip_prefix(&self.root).ok()?;
        let (spec, package_path) = Self::split_spec(relative_path)?;

        let package_id = PackageId::new_external(spec);
        let full_id = FullFileId::new(package_id, package_path.to_owned());

        Some(full_id)
    }
}

impl LocalProvider {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root: root_dir }
    }

    fn fs_path(&self, spec: &PackageSpec) -> PathBuf {
        let subdir = format!("{}/{}/{}/", spec.namespace, spec.name, spec.version);
        self.root.join(subdir)
    }

    /// Parse a spec from a path to a package directory, relative to the packaging root, returning
    /// the spec and package path.
    ///
    /// For example, given `preview/test/0.1.0/subdir/example.typ`, the spec `@preview/test:0.1.0`
    /// and path `subdir/example.typ` will be returned.
    fn split_spec(path: &Path) -> Option<(PackageSpec, VirtualPath)> {
        let mut components = path.components();

        let mut components_str = (&mut components)
            .filter(|component| matches!(component, Component::Normal(_)))
            .map(|component| component.as_os_str());

        let namespace = components_str.next()?.to_str()?;
        let name = components_str.next()?.to_str()?;
        let version = components_str.next()?.to_str()?;
        let spec_str = format!("@{namespace}/{name}:{version}");

        let spec = PackageSpec::from_str(&spec_str).ok()?;

        let package_path = VirtualPath::new(components.as_path());

        Some((spec, package_path))
    }
}

#[async_trait]
impl RepoRetrievalDest for LocalProvider {
    async fn store_tar_gz(
        &self,
        spec: &PackageSpec,
        package_tar_gz: impl AsyncBufRead + Unpin + Send,
    ) -> RepoResult<Package> {
        let path = self.fs_path(spec);
        let decompressed = self.decompress(package_tar_gz);
        self.unpack_to(decompressed, &path).await?;
        Ok(Package::new(
            LocalFs::path_to_uri(path).expect("should be absolute"),
        ))
    }
}

impl LocalProvider {
    fn decompress(&self, tar_gz: impl AsyncBufRead + Unpin) -> impl AsyncRead + Unpin {
        GzipDecoder::new(tar_gz)
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
