use std::collections::hash_map::Entry;
use std::collections::HashMap;

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

#[derive(Debug, Default)]
pub struct SourceManager {
    ids: HashMap<Url, SourceId>,
    sources: Vec<Source>,
}

impl SourceManager {
    pub fn uri_iter(&self) -> impl Iterator<Item = &Url> {
        self.ids.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Url, &Source)> {
        self.ids
            .iter()
            .map(|(uri, source_id)| (uri, self.get_source_by_id(*source_id)))
    }

    pub fn get_id_by_uri(&self, uri: &Url) -> Option<SourceId> {
        self.ids.get(uri).copied()
    }

    pub fn get_source_by_id(&self, id: SourceId) -> &Source {
        &self.sources[id.0 as usize]
    }

    pub fn get_source_by_uri(&self, uri: &Url) -> Option<&Source> {
        self.get_id_by_uri(uri).map(|id| self.get_source_by_id(id))
    }

    fn get_mut_source_by_id(&mut self, id: SourceId) -> &mut Source {
        &mut self.sources[id.0 as usize]
    }

    fn replace_source(&mut self, id: SourceId, replacement: Source) {
        *self.get_mut_source_by_id(id) = replacement;
    }

    fn get_next_id(&self) -> SourceId {
        SourceId(self.sources.len() as u16)
    }

    pub fn insert(&mut self, uri: &Url, text: String) {
        let next_id = self.get_next_id();

        match self.ids.entry(uri.clone()) {
            Entry::Occupied(entry) => {
                let existing_id = *entry.get();
                let source = Source::new(existing_id, uri, text);
                self.replace_source(existing_id, source);
            }
            Entry::Vacant(entry) => {
                entry.insert(next_id);
                let source = Source::new(next_id, uri, text);
                self.sources.push(source);
            }
        }
    }
}
