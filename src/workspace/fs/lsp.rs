use std::collections::{HashMap, HashSet};

use anyhow::anyhow;
use tower_lsp::lsp_types::{TextDocumentContentChangeEvent, Url};
use typst::syntax::Source;
use typst::util::Bytes;

use crate::config::PositionEncoding;
use crate::lsp_typst_boundary::LspRange;
use crate::workspace::package::manager::PackageManager;

use super::{FsError, FsResult, KnownUriProvider, ReadProvider};

/// Implements the Typst filesystem on source files provided by an LSP client
#[derive(Debug, Default)]
pub struct LspFs {
    files: HashMap<Url, Source>,
}

impl ReadProvider for LspFs {
    fn read_bytes(&self, uri: &Url, _: &PackageManager) -> FsResult<Bytes> {
        self.read_source_ref(uri)
            .map(|source| source.text().as_bytes().into())
    }

    fn read_source(&self, uri: &Url, _package_manager: &PackageManager) -> FsResult<Source> {
        self.read_source_ref(uri).cloned()
    }
}

impl KnownUriProvider for LspFs {
    fn known_uris(&self) -> HashSet<Url> {
        self.files.keys().cloned().collect()
    }
}

impl LspFs {
    pub fn open(
        &mut self,
        uri: Url,
        text: String,
        package_manager: &PackageManager,
    ) -> FsResult<()> {
        let full_id = package_manager.full_file_id(&uri)?;
        let source = Source::new(full_id.into(), text);
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

    fn read_source_ref(&self, uri: &Url) -> FsResult<&Source> {
        self.files
            .get(uri)
            .ok_or_else(|| FsError::NotProvided(anyhow!("URI not found")))
    }

    fn read_source_mut(&mut self, uri: &Url) -> FsResult<&mut Source> {
        self.files
            .get_mut(uri)
            .ok_or_else(|| FsError::NotProvided(anyhow!("URI not found")))
    }
}
