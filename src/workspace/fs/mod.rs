use tower_lsp::lsp_types::Url;
use typst::file::FileId;
use typst::syntax::Source;
use typst::util::Bytes;

pub mod local;
pub mod lsp;

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
    type Error;

    fn read_raw(&self, id: FileId) -> Result<Vec<u8>, Self::Error>;
    fn read_bytes(&self, id: FileId) -> Result<Bytes, Self::Error>;
    fn read_source(&self, id: FileId) -> Result<Source, Self::Error>;

    fn uri_to_id(&self, uri: &Url) -> Result<FileId, Self::Error>;
    fn id_to_uri(&self, id: FileId) -> Result<Url, Self::Error>;

    fn layered_over<Over: FsProvider>(self, over: Over) -> FsLayer<Self, Over>
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
    type Error = Over::Error;

    fn read_raw(&self, id: FileId) -> Result<Vec<u8>, Self::Error> {
        self.layer.read_raw(id).or_else(|_| self.over.read_raw(id))
    }

    fn read_bytes(&self, id: FileId) -> Result<Bytes, Self::Error> {
        self.layer
            .read_bytes(id)
            .or_else(|_| self.over.read_bytes(id))
    }

    fn read_source(&self, id: FileId) -> Result<Source, Self::Error> {
        self.layer
            .read_source(id)
            .or_else(|_| self.over.read_source(id))
    }

    fn uri_to_id(&self, uri: &Url) -> Result<FileId, Self::Error> {
        self.layer
            .uri_to_id(uri)
            .or_else(|_| self.over.uri_to_id(uri))
    }

    fn id_to_uri(&self, id: FileId) -> Result<Url, Self::Error> {
        self.layer
            .id_to_uri(id)
            .or_else(|_| self.over.id_to_uri(id))
    }
}
