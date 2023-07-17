use std::fs;
use std::path::{Path, PathBuf};

use elsa::sync::FrozenMap;
use once_cell::sync::OnceCell;
use tower_lsp::lsp_types::Url;
use typst::diag::{FileError, FileResult};
use typst::file::FileId;
use typst::util::{Bytes, PathExt};

use crate::lsp_typst_boundary::lsp_to_typst;

use super::source_manager::CacheableSource;

/// Implements the abstract Typst filesystem on the local filesystem. Finds project and package
/// files locally, downloading packages as needed to ensure their availability.
#[derive(Default)]
pub struct FileManager {
    files: FrozenMap<FileId, Box<File>>,
    project_root: PathBuf,
}

impl FileManager {
    pub fn file(&self, id: FileId) -> &File {
        self.files
            .get(&id) // don't take write lock unnecessarily
            .unwrap_or_else(|| self.files.insert(id, Box::default()))
    }

    pub fn file_mut(&mut self, id: FileId) -> &mut File {
        self.files.as_mut().entry(id).or_default()
    }

    pub fn read_bytes(&self, id: FileId) -> FileResult<Bytes> {
        self.read_raw(id).map(Bytes::from)
    }

    pub fn read_raw(&self, id: FileId) -> FileResult<Vec<u8>> {
        let path = self.id_to_path(id)?;
        Self::read_path_raw(&path)
    }

    /// Regular read from filesystem, returning a [`FileResult`] on failure
    pub fn read_path_raw(path: &Path) -> FileResult<Vec<u8>> {
        fs::read(&path).map_err(|err| FileError::from_io(err, path))
    }

    pub fn id_to_path(&self, id: FileId) -> FileResult<PathBuf> {
        match id.package() {
            None => self
                .project_root()
                .join_rooted(id.path())
                .ok_or_else(|| FileError::NotFound(id.path().to_owned())),
            Some(_package) => todo!("packages not yet implemented"),
        }
    }

    pub fn uri_to_id(&self, uri: &Url) -> anyhow::Result<FileId> {
        let root = self.project_root();
        lsp_to_typst::uri_to_file_id(uri, root)
    }

    pub fn all_file_ids(&self) -> Vec<FileId> {
        self.files.keys_cloned()
    }

    fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn clear(&mut self) {
        self.files
            .as_mut()
            .values_mut()
            .for_each(|file| file.invalidate());
    }
}

#[derive(Default)]
pub struct File {
    source: OnceCell<CacheableSource>,
    bytes: OnceCell<Bytes>,
}

impl File {
    fn from_source(source: CacheableSource) -> Self {
        Self {
            source: OnceCell::with_value(source),
            bytes: OnceCell::new(),
        }
    }

    pub fn cacheable_source(&self, id: FileId) -> &CacheableSource {
        self.source.get_or_init(|| CacheableSource::new_closed(id))
    }

    pub fn cacheable_source_mut(&mut self, id: FileId) -> &mut CacheableSource {
        self.source.get_or_init(|| CacheableSource::new_closed(id));
        self.source
            .get_mut()
            .expect("should be available just after init")
    }

    /// Determines if this file is a source file or not. That is, if `cacheable_source(_mut)` has
    /// even been called on it.
    pub fn is_source(&self) -> bool {
        self.source.get().is_some()
    }

    pub fn bytes(&self, id: FileId, file_manager: &FileManager) -> FileResult<&Bytes> {
        self.bytes.get_or_try_init(|| file_manager.read_bytes(id))
    }

    pub fn invalidate(&mut self) {
        self.source.take();
        self.bytes.take();
    }
}
