use std::collections::HashSet;
use std::fmt;

use elsa::sync::FrozenMap;
use once_cell::sync::OnceCell;
use tower_lsp::lsp_types::Url;
use tracing::trace;
use typst::eval::Bytes;
use typst::syntax::Source;

use crate::ext::PathExt;
use crate::workspace::package::manager::PackageManager;

use super::local::LocalFs;
use super::{FsResult, KnownUriProvider, ReadProvider, SourceSearcher};

#[derive(Default)]
pub struct Cache<Fs: ReadProvider> {
    entries: FrozenMap<Url, Box<CacheEntry>>,
    fs: Fs,
}

impl<Fs: ReadProvider> ReadProvider for Cache<Fs> {
    fn read_bytes(&self, uri: &Url, package_manager: &PackageManager) -> FsResult<Bytes> {
        self.read_bytes_ref(uri, package_manager).cloned()
    }

    fn read_source(&self, uri: &Url, package_manager: &PackageManager) -> FsResult<Source> {
        self.read_source_ref(uri, package_manager).cloned()
    }
}

impl<Fs: ReadProvider> KnownUriProvider for Cache<Fs> {
    fn known_uris(&self) -> HashSet<Url> {
        self.entries
            .keys_cloned()
            .into_iter()
            .filter(|key| LocalFs::uri_to_path(key).is_ok_and(|path| path.is_typst()))
            .collect()
    }
}

impl<Fs: ReadProvider> Cache<Fs> {
    /// Gives a reference to the wrapped [`ReadProvider`]. Note that this can cause cache
    /// invalidation errors if the inner reference writes to a cached file without the cache being
    /// notified.
    pub fn inner(&self) -> &Fs {
        &self.fs
    }

    pub fn read_bytes_ref(&self, uri: &Url, package_manager: &PackageManager) -> FsResult<&Bytes> {
        self.entry(uri.clone())
            .read_bytes(uri, &self.fs, package_manager)
    }

    pub fn read_source_ref(
        &self,
        uri: &Url,
        package_manager: &PackageManager,
    ) -> FsResult<&Source> {
        self.entry(uri.clone())
            .read_source(uri, &self.fs, package_manager)
    }

    pub fn cache_new(&mut self, uri: Url) {
        self.entry_mut(uri);
    }

    pub fn invalidate(&mut self, uri: Url) {
        self.entry_mut(uri).invalidate()
    }

    pub fn delete(&mut self, uri: &Url) {
        self.entries.as_mut().remove(uri);
    }

    pub fn clear(&mut self) {
        self.entries.as_mut().clear()
    }

    fn entry(&self, uri: Url) -> &CacheEntry {
        self.entries
            .get(&uri) // don't take write lock unnecessarily
            .unwrap_or_else(|| self.entries.insert(uri, Box::default()))
    }

    fn entry_mut(&mut self, uri: Url) -> &mut CacheEntry {
        self.entries.as_mut().entry(uri).or_default()
    }
}

impl<Fs: ReadProvider + SourceSearcher> Cache<Fs> {
    #[tracing::instrument(skip(self))]
    pub fn register_files(&mut self, root: &Url) -> FsResult<()> {
        for source in self.fs.search_sources(root)? {
            trace!(%source, "registering file");
            self.cache_new(source);
        }

        Ok(())
    }
}

impl<Fs: ReadProvider + fmt::Debug> fmt::Debug for Cache<Fs> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Cache")
            .field("entry_keys", &self.entries.keys_cloned())
            .field("fs", &self.fs)
            .finish()
    }
}

#[derive(Debug, Default)]
pub struct CacheEntry {
    source: OnceCell<Source>,
    bytes: OnceCell<Bytes>,
}

impl CacheEntry {
    pub fn read_bytes<Fs: ReadProvider>(
        &self,
        uri: &Url,
        fs: &Fs,
        package_manager: &PackageManager,
    ) -> FsResult<&Bytes> {
        self.bytes
            .get_or_try_init(|| fs.read_bytes(uri, package_manager))
    }

    pub fn read_source<Fs: ReadProvider>(
        &self,
        uri: &Url,
        fs: &Fs,
        package_manager: &PackageManager,
    ) -> FsResult<&Source> {
        self.source
            .get_or_try_init(|| fs.read_source(uri, package_manager))
    }

    pub fn invalidate(&mut self) {
        self.source.take();
        self.bytes.take();
    }
}
