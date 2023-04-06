use std::path::Path;

use comemo::Prehashed;
use parking_lot::Mutex;
use typst::diag::{FileError, FileResult};
use typst::eval::Library;
use typst::font::{Font, FontBook};
use typst::util::Buffer;
use typst::World;

use crate::workspace::font_manager::FontManager;
use crate::workspace::resource_manager::ResourceManager;
use crate::workspace::source_manager::{SourceId, SourceManager};

use super::{typst_to_lsp, TypstSource, TypstSourceId};

pub struct LspWorldBuilder {
    library: Prehashed<Library>,
    font_manager: FontManager,
}

impl LspWorldBuilder {
    pub fn build<'a>(
        &'a self,
        main_id: SourceId,
        sources: &'a SourceManager,
        resources: &'a mut ResourceManager,
    ) -> LspWorld<'a> {
        LspWorld {
            main_id,
            library: &self.library,
            sources,
            resources: Mutex::new(resources),
            font_manager: &self.font_manager,
        }
    }
}

impl Default for LspWorldBuilder {
    fn default() -> Self {
        Self {
            library: Prehashed::new(typst_library::build()),
            font_manager: FontManager::builder().with_system().with_embedded().build(),
        }
    }
}

pub struct LspWorld<'a> {
    main_id: SourceId,
    library: &'a Prehashed<Library>,
    sources: &'a SourceManager,
    resources: Mutex<&'a mut ResourceManager>,
    font_manager: &'a FontManager,
}

impl World for LspWorld<'_> {
    fn library(&self) -> &Prehashed<Library> {
        self.library
    }

    fn main(&self) -> &TypstSource {
        self.sources.get_source_by_id(self.main_id).as_ref()
    }

    fn resolve(&self, typst_path: &Path) -> FileResult<TypstSourceId> {
        let lsp_uri = typst_to_lsp::path_to_uri(typst_path).unwrap();
        let lsp_id = self.sources.get_id_by_uri(&lsp_uri);
        match lsp_id {
            Some(lsp_id) => Ok(lsp_id.into()),
            None => Err(FileError::NotFound(typst_path.to_owned())),
        }
    }

    fn source(&self, typst_id: typst::syntax::SourceId) -> &TypstSource {
        let lsp_source = self.sources.get_source_by_id(typst_id.into());
        lsp_source.as_ref()
    }

    fn book(&self) -> &Prehashed<FontBook> {
        self.font_manager.book()
    }

    fn font(&self, id: usize) -> Option<Font> {
        let mut resources = self.resources.lock();
        self.font_manager.font(id, &mut resources)
    }

    fn file(&self, typst_path: &Path) -> FileResult<Buffer> {
        let mut resources = self.resources.lock();
        let lsp_uri = typst_to_lsp::path_to_uri(typst_path).unwrap();
        let lsp_resource = resources.get_or_insert_resource(lsp_uri)?;
        Ok(lsp_resource.into())
    }
}
