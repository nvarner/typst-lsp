use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::mem;

use tower_lsp::lsp_types::Url;

use crate::lsp_typst_boundary::TypstSourceId;

use super::source::Source;

#[derive(Debug, Clone, Copy)]
pub struct SourceId(u16);

impl From<TypstSourceId> for SourceId {
    fn from(typst_id: TypstSourceId) -> Self {
        Self(typst_id.into_u16())
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
    ClosedUnmodified(Source),
    ClosedModified(Url),
}

impl InnerSource {
    pub fn get_source(&self) -> Option<&Source> {
        match self {
            Self::Open(source) | Self::ClosedUnmodified(source) => Some(source),
            Self::ClosedModified(_) => None,
        }
    }

    pub fn get_mut_source(&mut self) -> Option<&mut Source> {
        match self {
            Self::Open(source) | Self::ClosedUnmodified(source) => Some(source),
            Self::ClosedModified(_) => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct SourceManager {
    ids: HashMap<Url, SourceId>,
    sources: Vec<InnerSource>,
}

impl SourceManager {
    pub fn uri_iter(&self) -> impl Iterator<Item = &Url> {
        self.ids.keys()
    }

    pub fn get_id_by_uri(&self, uri: &Url) -> Option<SourceId> {
        self.ids.get(uri).copied()
    }

    /// Gets a source which is known to be open in the LSP client
    pub fn get_open_source_by_id(&self, id: SourceId) -> &Source {
        self.sources[id.0 as usize]
            .get_source()
            .expect("open source should exist")
    }

    pub fn get_mut_open_source_by_id(&mut self, id: SourceId) -> &mut Source {
        self.sources[id.0 as usize]
            .get_mut_source()
            .expect("open source should exist")
    }

    fn get_next_id(&self) -> SourceId {
        SourceId(self.sources.len() as u16)
    }

    fn get_mut_inner_source(&mut self, id: SourceId) -> &mut InnerSource {
        &mut self.sources[id.0 as usize]
    }

    pub fn insert_open(&mut self, uri: &Url, text: String) {
        let next_id = self.get_next_id();

        match self.ids.entry(uri.clone()) {
            Entry::Occupied(entry) => {
                let existing_id = *entry.get();
                let source = Source::new(existing_id, uri, text);
                *self.get_mut_inner_source(existing_id) = InnerSource::Open(source);
            }
            Entry::Vacant(entry) => {
                entry.insert(next_id);
                let source = Source::new(next_id, uri, text);
                self.sources.push(InnerSource::Open(source));
            }
        }
    }

    pub fn close(&mut self, uri: &Url) {
        if let Some(id) = self.get_id_by_uri(uri) {
            let inner_source = self.get_mut_inner_source(id);
            if let InnerSource::Open(source) = inner_source {
                let source = mem::replace(source, Source::new_detached());
                *inner_source = InnerSource::ClosedUnmodified(source);
            }
        }
    }

    pub fn invalidate_closed(&mut self, uri: Url) {
        if let Some(id) = self.get_id_by_uri(&uri) {
            let inner_source = self.get_mut_inner_source(id);
            if let InnerSource::ClosedUnmodified(_) = *inner_source {
                *inner_source = InnerSource::ClosedModified(uri);
            }
        }
    }
}
