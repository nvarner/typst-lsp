use std::fs;
use std::path::{Path, PathBuf};

use elsa::sync::FrozenMap;
use once_cell::sync::OnceCell;
use typst::diag::{FileError, FileResult};
use typst::file::FileId;
use typst::util::{Bytes, PathExt};

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

    pub fn get_file_mut(&mut self, id: FileId) -> &mut File {
        self.files.as_mut().entry(id).or_default()
    }

    pub fn read_bytes(&self, id: FileId) -> FileResult<Bytes> {
        self.read_raw(id).map(Bytes::from)
    }

    pub fn read_raw(&self, id: FileId) -> FileResult<Vec<u8>> {
        let path = self.resolve_path(id)?;
        Self::read_path_raw(&path)
    }

    /// Regular read from filesystem, returning a [`FileResult`] on failure
    pub fn read_path_raw(path: &Path) -> FileResult<Vec<u8>> {
        fs::read(&path).map_err(|err| FileError::from_io(err, path))
    }

    fn resolve_path(&self, id: FileId) -> FileResult<PathBuf> {
        match id.package() {
            None => self
                .project_root
                .join_rooted(id.path())
                .ok_or_else(|| FileError::NotFound(id.path().to_owned())),
            Some(package) => todo!("packages not yet implemented"),
        }
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
        self.source.get_or_init(|| CacheableSource::closed(id))
    }

    pub fn bytes(&self, id: FileId, file_manager: &FileManager) -> FileResult<&Bytes> {
        self.bytes.get_or_try_init(|| file_manager.read_bytes(id))
    }

    pub fn invalidate(&mut self) {
        self.source.take();
        self.bytes.take();
    }
}
