use tower_lsp::lsp_types::Url;
use typst::diag::FileResult;
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

pub mod local;

/// Implements the Typst filesystem for a single project.
///
/// The Typst filesystem is based on a project (what the user is currently working on) and packages
/// (which can be downloaded and imported as dependencies). There is always exactly one project.
/// Multiple projects should be represented by multiple instances of implementors.
///
/// Implementations provide access to project and package files, downloading packages as needed to
/// ensure their availability. They must also provide conversions between LSP URIs and Typst file
/// IDs, since this mapping is expected to be based on the filesystem.
pub trait FsProvider {
    fn read_raw(&self, id: FileId) -> FileResult<Vec<u8>>;
    fn read_bytes(&self, id: FileId) -> FileResult<Bytes>;
    fn read_source(&self, id: FileId) -> FileResult<Source>;

    fn uri_to_id(&self, uri: &Url) -> FileResult<FileId>;
    fn id_to_uri(&self, id: FileId) -> FileResult<Url>;

    fn layer_over<Over: FsProvider>(self, over: Over) -> FsLayer<Self, Over>
    where
        Self: Sized,
    {
        FsLayer { layer: self, over }
    }
}

/// Composes two `FsProviders`, layering one "over" another. When the upper provider fails, it falls
/// back to the underlying provider.
pub struct FsLayer<Layer: FsProvider, Over: FsProvider> {
    layer: Layer,
    over: Over,
}

impl<Layer: FsProvider, Over: FsProvider> FsProvider for FsLayer<Layer, Over> {
    fn read_raw(&self, id: FileId) -> FileResult<Vec<u8>> {
        self.layer.read_raw(id).or_else(|_| self.over.read_raw(id))
    }

    fn read_bytes(&self, id: FileId) -> FileResult<Bytes> {
        self.layer
            .read_bytes(id)
            .or_else(|_| self.over.read_bytes(id))
    }

    fn read_source(&self, id: FileId) -> FileResult<Source> {
        self.layer
            .read_source(id)
            .or_else(|_| self.over.read_source(id))
    }

    fn uri_to_id(&self, uri: &Url) -> FileResult<FileId> {
        self.layer
            .uri_to_id(uri)
            .or_else(|_| self.over.uri_to_id(uri))
    }

    fn id_to_uri(&self, id: FileId) -> FileResult<Url> {
        self.layer
            .id_to_uri(id)
            .or_else(|_| self.over.id_to_uri(id))
    }
}
