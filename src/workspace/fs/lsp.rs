use std::collections::HashMap;

use tower_lsp::lsp_types::Url;
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

use super::FsProvider;

/// Implements the Typst filesystem on source files provided by an LSP client. Intended to be used
/// as a layer on top of another provider, since the LSP does not provide a mapping between URIs and
/// file IDs.
#[derive(Default)]
pub struct LspFs {
    files: HashMap<FileId, Source>,
}

impl FsProvider for LspFs {
    type Error = ();

    fn read_raw(&self, id: FileId) -> Result<Vec<u8>, ()> {
        self.read_bytes(id).map(|bytes| bytes.to_vec())
    }

    fn read_bytes(&self, id: FileId) -> Result<Bytes, ()> {
        self.read_source(id)
            .map(|source| source.text().as_bytes())
            .map(Bytes::from)
    }

    fn read_source(&self, id: FileId) -> Result<Source, ()> {
        self.read_source_ref(id).cloned()
    }

    fn uri_to_id(&self, uri: &Url) -> Result<FileId, ()> {
        Err(())
    }

    fn id_to_uri(&self, id: FileId) -> Result<Url, ()> {
        Err(())
    }
}

impl LspFs {
    fn read_source_ref(&self, id: FileId) -> Result<&Source, ()> {
        self.files.get(&id).ok_or(())
    }
}
