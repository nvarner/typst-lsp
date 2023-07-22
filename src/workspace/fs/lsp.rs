use std::collections::HashMap;

use tower_lsp::lsp_types::{TextDocumentContentChangeEvent, Url};
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use crate::config::PositionEncoding;
use crate::lsp_typst_boundary::LspRange;

use super::FsProvider;

/// Implements the Typst filesystem on source files provided by an LSP client. Intended to be
/// composed with another provider, since the LSP does not provide a mapping between URIs and file
/// IDs.
#[derive(Default)]
pub struct LspFs {
    files: HashMap<FileId, Source>,
}

impl LspFs {
    pub fn open(&mut self, id: FileId, text: String) {
        let source = Source::new(id, text);
        self.files.insert(id, source);
    }

    pub fn close(&mut self, id: FileId) {
        self.files.remove(&id);
    }

    pub fn edit(
        &mut self,
        id: FileId,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
        position_encoding: PositionEncoding,
    ) {
        let Ok(source) = self.read_source_mut(id) else { return };
        changes
            .into_iter()
            .for_each(|change| Self::apply_one_change(source, change, position_encoding));
    }

    fn apply_one_change(
        source: &mut Source,
        change: TextDocumentContentChangeEvent,
        position_encoding: PositionEncoding,
    ) {
        let replacement = change.text;

        match change.range {
            Some(lsp_range) => {
                let range = LspRange::new(lsp_range, position_encoding).to_range_on(source);
                source.edit(range, &replacement);
            }
            None => source.replace(replacement),
        }
    }

    pub fn clear(&mut self) {
        self.files.clear();
    }

    fn read_source_ref(&self, id: FileId) -> Result<&Source, ()> {
        self.files.get(&id).ok_or(())
    }

    fn read_source_mut(&mut self, id: FileId) -> Result<&mut Source, ()> {
        self.files.get_mut(&id).ok_or(())
    }
}

impl FsProvider for LspFs {
    type Error = ();

    fn read_raw(&self, id: FileId) -> Result<Vec<u8>, ()> {
        self.read_bytes(id).map(|bytes| bytes.to_vec())
    }

    fn read_bytes(&self, id: FileId) -> Result<Bytes, ()> {
        self.read_source(id)
            .map(|source| source.text().as_bytes().into())
    }

    fn read_source(&self, id: FileId) -> Result<Source, ()> {
        self.read_source_ref(id).cloned()
    }

    fn uri_to_id(&self, _: &Url) -> Result<FileId, ()> {
        Err(())
    }

    fn id_to_uri(&self, _: FileId) -> Result<Url, ()> {
        Err(())
    }
}
