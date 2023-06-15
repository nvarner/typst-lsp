use std::{fmt, fs, io, mem};

use elsa::sync::FrozenVec;
use indexmap::IndexSet;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use tower_lsp::lsp_types::Url;
use typst::diag::{FileError, FileResult};
use typst::syntax::SourceId;
use walkdir::WalkDir;

use crate::lsp_typst_boundary::{lsp_to_typst, typst_to_lsp};

use super::source::Source;

#[derive(Debug)]
enum InnerSource {
    Open(Source),
    Closed(OnceCell<Source>),
}

impl InnerSource {
    pub fn get_source(&self) -> Option<&Source> {
        match self {
            Self::Open(source) => Some(source),
            Self::Closed(cell) => cell.get(),
        }
    }

    pub fn get_mut_source(&mut self) -> Option<&mut Source> {
        match self {
            Self::Open(source) => Some(source),
            Self::Closed(cell) => cell.get_mut(),
        }
    }
}

/// Provides access to [`Source`]s via [`SourceId`]s and [`Url`]s
#[derive(Default)]
pub struct SourceManager {
    ids: RwLock<IndexSet<Url>>,
    sources: FrozenVec<Box<InnerSource>>,
}

impl SourceManager {
    /// Get the URIs of all sources which have been seen
    pub fn get_uris(&self) -> Vec<Url> {
        self.ids.read().iter().cloned().collect()
    }

    pub fn get_id_by_uri(&self, uri: &Url) -> Option<SourceId> {
        self.ids
            .read()
            .get_index_of(uri)
            .map(|id| SourceId::from_u16(id as u16))
    }

    fn get_inner_source(&self, id: SourceId) -> &InnerSource {
        // We treat all `SourceId`s as valid
        self.sources.get(id.as_u16() as usize).unwrap()
    }

    fn get_mut_inner_source(&mut self, id: SourceId) -> &mut InnerSource {
        // We treat all `SourceId`s as valid
        self.sources.as_mut().get_mut(id.as_u16() as usize).unwrap()
    }

    fn get_cached_source_by_id(&self, id: SourceId) -> Option<&Source> {
        self.get_inner_source(id).get_source()
    }

    /// Gets a source which is known to be open in the LSP client
    pub fn get_open_source_by_id(&self, id: SourceId) -> &Source {
        self.get_cached_source_by_id(id)
            .expect("open source should exist")
    }

    pub fn get_mut_open_source_by_id(&mut self, id: SourceId) -> &mut Source {
        self.get_mut_inner_source(id)
            .get_mut_source()
            .expect("open source should exist")
    }

    pub fn insert_open(&mut self, uri: &Url, text: String) -> anyhow::Result<()> {
        let ids = self.ids.get_mut();
        let (index, uri_is_new) = ids.insert_full(uri.clone());
        let id = SourceId::from_u16(index as u16);
        let source = Source::new(id, uri, text)?;

        if uri_is_new {
            *self.get_mut_inner_source(id) = InnerSource::Open(source);
        } else {
            self.sources.push(Box::new(InnerSource::Open(source)));
        }

        Ok(())
    }

    pub fn close(&mut self, uri: &Url) {
        if let Some(id) = self.get_id_by_uri(uri) {
            let inner_source = self.get_mut_inner_source(id);
            if let InnerSource::Open(source) = inner_source {
                let source = mem::replace(source, Source::new_detached());
                *inner_source = InnerSource::Closed(OnceCell::with_value(source));
            }
        }
    }

    pub fn invalidate_closed(&mut self, uri: &Url) {
        if let Some(id) = self.get_id_by_uri(uri) {
            let inner_source = self.get_mut_inner_source(id);
            if let InnerSource::Closed(cell) = inner_source {
                cell.take();
            }
        }
    }

    fn read_source_from_file(id: SourceId, uri: &Url) -> FileResult<Source> {
        let path = lsp_to_typst::uri_to_path(uri).map_err(|_| FileError::Other)?;
        let text = fs::read_to_string(&path).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => FileError::NotFound(path),
            io::ErrorKind::PermissionDenied => FileError::AccessDenied,
            _ => FileError::Other,
        })?;
        Source::new(id, uri, text).map_err(|_| FileError::Other)
    }

    pub fn cache(&self, uri: Url) -> FileResult<SourceId> {
        let mut ids = self.ids.write();
        let (index, uri_is_new) = ids.insert_full(uri.clone());
        let id = SourceId::from_u16(index as u16);

        if uri_is_new {
            let source = Self::read_source_from_file(id, &uri)?;
            self.sources
                .push(Box::new(InnerSource::Closed(OnceCell::with_value(source))));
        } else {
            let inner_source = self.get_inner_source(id);
            if let InnerSource::Closed(cell) = inner_source {
                cell.get_or_try_init(|| Self::read_source_from_file(id, &uri))?;
            }
        }

        Ok(id)
    }

    pub fn register_workspace_files(&self, workspace: &Url) -> anyhow::Result<()> {
        let workspace_path = lsp_to_typst::uri_to_path(workspace)?;
        let walker = WalkDir::new(workspace_path).into_iter();
        for entry in walker.filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let path = entry.path();
                let ext = path.extension().unwrap_or_default();
                if ext != "typ" {
                    continue;
                }
                let uri = typst_to_lsp::path_to_uri(path)?;
                self.cache(uri)?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for SourceManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SourceManager").finish_non_exhaustive()
    }
}
