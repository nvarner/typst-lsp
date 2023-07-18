use tower_lsp::lsp_types::Url;
use typst::diag::FileResult;
use typst::file::FileId;
use typst::util::Bytes;

pub mod cache;
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
pub trait TypstFs {
    fn read_raw(&self, id: FileId) -> FileResult<Vec<u8>>;
    fn uri_to_id(&self, uri: &Url) -> FileResult<FileId>;
    fn id_to_uri(&self, id: FileId) -> FileResult<Url>;

    fn read_bytes(&self, id: FileId) -> FileResult<Bytes> {
        self.read_raw(id).map(Bytes::from)
    }
}
