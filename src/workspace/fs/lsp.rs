use std::collections::HashMap;

use tower_lsp::lsp_types::{TextDocumentContentChangeEvent, Url};
use typst::diag::FileResult;
use typst::syntax::Source;
use typst::util::Bytes;

use crate::config::PositionEncoding;
use crate::lsp_typst_boundary::LspRange;
use crate::workspace::project::manager::ProjectManager;

use super::ReadProvider;

/// Implements the Typst filesystem on source files provided by an LSP client
#[derive(Default)]
pub struct LspFs {
    files: HashMap<Url, Source>,
}

impl LspFs {
    pub fn open(
        &mut self,
        uri: Url,
        text: String,
        project_manager: &ProjectManager,
    ) -> FileResult<()> {
        let id = project_manager.uri_to_id(&uri)?;
        let source = Source::new(id, text);
        self.files.insert(uri, source);
        Ok(())
    }

    pub fn close(&mut self, uri: &Url) {
        self.files.remove(uri);
    }

    pub fn edit(
        &mut self,
        uri: &Url,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
        position_encoding: PositionEncoding,
    ) {
        let Ok(source) = self.read_source_mut(uri) else { return };
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
                let range = LspRange::new(lsp_range, position_encoding).into_range_on(source);
                source.edit(range, &replacement);
            }
            None => source.replace(replacement),
        }
    }

    pub fn clear(&mut self) {
        self.files.clear();
    }

    fn read_source_ref(&self, uri: &Url) -> Result<&Source, ()> {
        self.files.get(uri).ok_or(())
    }

    fn read_source_mut(&mut self, uri: &Url) -> Result<&mut Source, ()> {
        self.files.get_mut(uri).ok_or(())
    }
}

impl ReadProvider for LspFs {
    type Error = ();

    fn read_bytes(&self, uri: &Url) -> Result<Bytes, ()> {
        self.read_source_ref(uri)
            .map(|source| source.text().as_bytes().into())
    }

    fn read_source(&self, uri: &Url, _project_manager: &ProjectManager) -> Result<Source, ()> {
        self.read_source_ref(uri).cloned()
    }
}
