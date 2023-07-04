use elsa::sync::FrozenMap;
use once_cell::sync::OnceCell;
use typst::file::FileId;
use typst::util::Bytes;

use crate::lsp_typst_boundary::TypstSource;

pub struct FileManager {
    files: FrozenMap<FileId, Box<File>>,
}

impl FileManager {
    pub fn get(&self, id: FileId) -> &File {
        self.files
            .get(&id) // don't take write lock unnecessarily
            .unwrap_or_else(|| self.files.insert(id, Box::default()))
    }

    pub fn get_mut(&mut self, id: FileId) -> &mut File {
        self.files.as_mut().entry(id).or_default()
    }
}

#[derive(Default)]
pub struct File {
    source: CachableSource,
    bytes: CachableBytes,
}

impl File {
    fn from_source(source: CachableSource) -> Self {
        Self {
            source,
            bytes: CachableBytes::empty(),
        }
    }
}

#[derive(Default)]
enum CachableSource {
    /// Cache has never been initialized for this source. This is used when a [`File`] is not
    /// actually a source (e.g. for fonts).
    #[default]
    Uninit,
    Open(TypstSource),
    Closed(FileId, OnceCell<TypstSource>),
}

impl CachableSource {
    pub fn closed(id: FileId) -> Self {
        Self::Closed(id, OnceCell::new())
    }

    pub fn get_source(&self) -> Option<&TypstSource> {
        match self {
            Self::Uninit => None,
            Self::Open(source) => Some(source),
            Self::Closed(_, cell) => cell.get(),
        }
    }

    pub fn get_mut_source(&mut self) -> Option<&mut TypstSource> {
        match self {
            Self::Uninit => None,
            Self::Open(source) => Some(source),
            Self::Closed(_, cell) => cell.get_mut(),
        }
    }
}

#[derive(Default)]
struct CachableBytes(OnceCell<Bytes>);

impl CachableBytes {
    pub fn empty() -> Self {
        Self::default()
    }
}
