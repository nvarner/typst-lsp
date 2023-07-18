use elsa::sync::FrozenMap;
use once_cell::sync::OnceCell;
use tower_lsp::lsp_types::Url;
use typst::diag::FileResult;
use typst::file::FileId;
use typst::util::Bytes;

use super::TypstFs;

#[derive(Default)]
pub struct FsCache<Fs: TypstFs> {
    entries: FrozenMap<FileId, Box<CacheEntry>>,
    fs: Fs,
}

impl<Fs: TypstFs> FsCache<Fs> {
    pub fn new(fs: Fs) -> Self {
        Self {
            entries: Default::default(),
            fs,
        }
    }

    pub fn read_bytes_ref(&self, id: FileId) -> FileResult<&Bytes> {
        self.entry(id).read(id, &self.fs)
    }

    pub fn invalidate(&mut self, id: FileId) {
        self.entry_mut(id).invalidate()
    }

    pub fn clear(&mut self) {
        self.entries.as_mut().clear()
    }

    fn entry(&self, id: FileId) -> &CacheEntry {
        self.entries
            .get(&id) // don't take write lock unnecessarily
            .unwrap_or_else(|| self.entries.insert(id, Box::default()))
    }

    fn entry_mut(&mut self, id: FileId) -> &mut CacheEntry {
        self.entries.as_mut().entry(id).or_default()
    }
}

impl<Fs: TypstFs> TypstFs for FsCache<Fs> {
    fn read_raw(&self, id: FileId) -> FileResult<Vec<u8>> {
        self.read_bytes_ref(id).map(|bytes| bytes.to_vec())
    }

    fn uri_to_id(&self, uri: &Url) -> FileResult<FileId> {
        self.fs.uri_to_id(uri)
    }

    fn id_to_uri(&self, id: FileId) -> FileResult<Url> {
        self.fs.id_to_uri(id)
    }

    fn read_bytes(&self, id: FileId) -> FileResult<Bytes> {
        self.read_bytes_ref(id).cloned()
    }
}

#[derive(Default)]
pub struct CacheEntry(OnceCell<Bytes>);

impl CacheEntry {
    pub fn read(&self, id: FileId, fs: &impl TypstFs) -> FileResult<&Bytes> {
        self.0.get_or_try_init(|| fs.read_bytes(id))
    }

    pub fn invalidate(&mut self) {
        self.0.take();
    }
}
