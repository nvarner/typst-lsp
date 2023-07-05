use typst::diag::FileResult;
use typst::file::FileId;
use typst::util::Bytes;

use super::file_manager::FileManager;

pub trait ResourceManager {
    fn resource(self, id: FileId) -> FileResult<Bytes>;
}

impl<'a> ResourceManager for &'a FileManager {
    fn resource(self, id: FileId) -> FileResult<Bytes> {
        self.file(id).bytes(id, self).cloned()
    }
}
