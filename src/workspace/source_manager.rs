use once_cell::sync::OnceCell;
use tower_lsp::lsp_types::Url;
use tracing::{trace, warn};
use typst::diag::{FileError, FileResult};
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;
use walkdir::WalkDir;

use crate::lsp_typst_boundary::{lsp_to_typst, typst_to_lsp};

use super::fs::FsProvider;

/// Provides access to [`Source`] documents via [`FileId`]s
///
/// A document can be open or closed. "Open" and "closed" correspond to the document's reported
/// state in the LSP client.
pub struct SourceManager {}

// impl FsCache {
//     #[tracing::instrument(skip(self))]
//     fn source(&self, id: FileId) -> FileResult<&Source> {
//         self.file(id).cacheable_source(id).read(self)
//     }

//     #[tracing::instrument(skip(self))]
//     fn source_mut(&mut self, id: FileId) -> FileResult<&mut Source> {
//         self.file_mut(id).cacheable_source_mut(id).read_mut(self)
//     }

//     #[tracing::instrument(skip(self, text))]
//     fn open(&mut self, id: FileId, text: String) {
//         self.file_mut(id).cacheable_source_mut(id).open(text)
//     }

//     #[tracing::instrument(skip(self))]
//     fn close(&mut self, id: FileId) {
//         self.file_mut(id).cacheable_source_mut(id).close();
//     }

//     /// Get the file IDs of all sources
//     #[tracing::instrument(skip(self))]
//     fn all_file_ids(&self) -> Vec<FileId> {
//         self.all_file_ids()
//             .into_iter()
//             .filter(|id| self.file(*id).is_source())
//             .collect()
//     }

//     /// Add all Typst files in `workspace` to the `SourceManager`, caching them as needed
//     #[tracing::instrument(skip_all, fields(%workspace))]
//     fn register_workspace_files(&mut self, workspace: &Url) -> anyhow::Result<()> {
//         let workspace_path = lsp_to_typst::uri_to_path(workspace)?;

//         WalkDir::new(&workspace_path)
//             .into_iter()
//             .filter_map(Result::ok)
//             .filter(|entry| entry.file_type().is_file())
//             .filter(|file| file.path().extension().map_or(false, |ext| ext == "typ"))
//             .map(|file| typst_to_lsp::path_to_uri(file.path()))
//             .filter_map(|x| x.map_err(|err| warn!(?err, "could not get uri")).ok())
//             .map(|uri| lsp_to_typst::uri_to_file_id(&uri, &workspace_path))
//             .filter_map(|x| x.map_err(|err| warn!(?err, "could not get file id")).ok())
//             .inspect(|id| trace!(%id, "registering file"))
//             .map(|id| self.source(id))
//             .filter_map(|x| x.map_err(|err| warn!(?err, "could not register file")).ok())
//             .for_each(|_| ());

//         Ok(())
//     }
// }

#[derive(Default)]
pub struct CacheEntry {
    source: OnceCell<CacheableSource>,
    bytes: OnceCell<Bytes>,
}

impl CacheEntry {
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

    pub fn bytes(&self, id: FileId, typst_fs: &impl FsProvider) -> FileResult<&Bytes> {
        self.bytes.get_or_try_init(|| typst_fs.read_bytes(id))
    }

    pub fn invalidate(&mut self) {
        self.source.take();
        self.bytes.take();
    }
}

pub enum CacheableSource {
    Open(FileId, Source),
    Closed(FileId, OnceCell<Source>),
}

impl CacheableSource {
    pub fn new_closed(id: FileId) -> Self {
        Self::Closed(id, OnceCell::new())
    }

    fn new_closed_cached(id: FileId, source: Source) -> Self {
        Self::Closed(id, OnceCell::with_value(source))
    }

    fn new_open(id: FileId, source: Source) -> Self {
        Self::Open(id, source)
    }

    pub fn open(&mut self, text: String) {
        if let Self::Closed(id, _) = self {
            let source = Source::new(*id, text);
            *self = Self::new_open(*id, source);
        }
    }

    pub fn close(&mut self) {
        if let Self::Open(id, source) = self {
            *self = Self::new_closed_cached(*id, source.clone());
        }
    }

    /// Read the underlying source, or from cache if available
    pub fn read<'a, 'b>(&'a self, project_fs: &'b impl FsProvider) -> FileResult<&'a Source> {
        match self {
            Self::Open(_, source) => Ok(source),
            Self::Closed(id, cell) => {
                cell.get_or_try_init(|| Self::read_from_file(*id, project_fs))
            }
        }
    }

    /// Read the underlying source, or from cache if available
    pub fn read_mut<'a, 'b>(
        &'a mut self,
        project_fs: &'b impl FsProvider,
    ) -> FileResult<&'a mut Source> {
        match self {
            Self::Open(_, source) => Ok(source),
            Self::Closed(id, cell) => {
                cell.get_or_try_init(|| Self::read_from_file(*id, project_fs))?;
                Ok(cell.get_mut().expect("should be available just after init"))
            }
        }
    }

    fn read_from_file(id: FileId, project_fs: &impl FsProvider) -> FileResult<Source> {
        let raw = project_fs.read_raw(id)?;
        let text = String::from_utf8(raw).map_err(|err| {
            warn!(?err, "failed to convert raw bytes into UTF-8 string");
            FileError::InvalidUtf8
        })?;
        Ok(Source::new(id, text))
    }
}

// impl FsCache {
//     fn get_inner_source(&self, id: SourceId) -> &InnerSource {
//         // We treat all `SourceId`s as valid
//         self.sources.get(id.as_u16() as usize).unwrap()
//     }

//     fn get_mut_inner_source(&mut self, id: SourceId) -> &mut InnerSource {
//         // We treat all `SourceId`s as valid
//         self.sources.as_mut().get_mut(id.as_u16() as usize).unwrap()
//     }
// }

#[derive(Debug)]
enum InnerSource {
    Open(Source),
    Closed(OnceCell<Source>, Url),
}

impl InnerSource {
    pub fn closed(source: Source, uri: Url) -> Self {
        Self::Closed(OnceCell::with_value(source), uri)
    }

    pub fn get_source(&self) -> Option<&Source> {
        match self {
            Self::Open(source) => Some(source),
            Self::Closed(cell, _) => cell.get(),
        }
    }

    pub fn get_or_init_source(&self, id: SourceId) -> FileResult<&Source> {
        match self {
            Self::Open(source) => Ok(source),
            Self::Closed(cell, uri) => cell.get_or_try_init(|| Source::read_from_file(id, uri)),
        }
    }

    pub fn get_mut_source(&mut self) -> Option<&mut Source> {
        match self {
            Self::Open(source) => Some(source),
            Self::Closed(cell, _) => cell.get_mut(),
        }
    }
}
