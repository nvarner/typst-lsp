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
}
