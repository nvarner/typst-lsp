use std::collections::hash_map::Entry;
use std::{fmt, fs, io, mem};

use elsa::sync::{FrozenMap, FrozenVec};
use once_cell::sync::OnceCell;
use tower_lsp::lsp_types::Url;
use typst::diag::{FileError, FileResult};
use walkdir::WalkDir;

use crate::lsp_typst_boundary::{lsp_to_typst, typst_to_lsp, TypstSourceId};

use super::source::Source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceId(u16);

impl From<TypstSourceId> for SourceId {
    fn from(typst_id: TypstSourceId) -> Self {
        Self(typst_id.as_u16())
    }
}

impl From<SourceId> for TypstSourceId {
    fn from(lsp_id: SourceId) -> Self {
        Self::from_u16(lsp_id.0)
    }
}

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

#[derive(Default)]
pub struct SourceManager {
    ids: FrozenMap<Url, SourceId>,
    sources: FrozenVec<Box<InnerSource>>,
}

impl SourceManager {
    pub fn get_uris(&self) -> Vec<Url> {
        self.ids.keys_cloned()
    }

    pub fn get_id_by_uri(&self, uri: &Url) -> Option<SourceId> {
        self.ids.get_copy(uri)
    }

    fn get_inner_source(&self, id: SourceId) -> &InnerSource {
        self.sources.get(id.0 as usize).unwrap()
    }

    fn get_mut_inner_source(&mut self, id: SourceId) -> &mut InnerSource {
        self.sources.as_mut().get_mut(id.0 as usize).unwrap()
    }

    /// Gets a source which is known to be open in the LSP client
    pub fn get_open_source_by_id(&self, id: SourceId) -> &Source {
        self.get_inner_source(id)
            .get_source()
            .expect("open source should exist")
    }

    pub fn get_mut_open_source_by_id(&mut self, id: SourceId) -> &mut Source {
        self.get_mut_inner_source(id)
            .get_mut_source()
            .expect("open source should exist")
    }

    fn get_next_id(&self) -> SourceId {
        SourceId(self.sources.len() as u16)
    }

    pub fn insert_open(&mut self, uri: &Url, text: String) -> anyhow::Result<()> {
        let next_id = self.get_next_id();

        match self.ids.as_mut().entry(uri.clone()) {
            Entry::Occupied(entry) => {
                let existing_id = *entry.get();
                let source = Source::new(existing_id, uri, text)?;
                *self.get_mut_inner_source(existing_id) = InnerSource::Open(source);
            }
            Entry::Vacant(entry) => {
                entry.insert(next_id);
                let source = Source::new(next_id, uri, text)?;
                self.sources.push(Box::new(InnerSource::Open(source)));
            }
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
        let next_id = self.get_next_id();

        let id = self.ids.get_copy_or_insert(uri.clone(), next_id);

        // TODO: next_id could expire before the new source is inserted; lock across everything, or
        // use a more appropriate structure which handles that automatically
        if id == next_id {
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
