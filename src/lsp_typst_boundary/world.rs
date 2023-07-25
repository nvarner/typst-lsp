use comemo::Prehashed;
use tracing::{error, warn};
use typst::diag::{EcoString, FileResult};
use typst::eval::{Datetime, Library};
use typst::file::{FileId, PackageSpec};
use typst::font::{Font, FontBook};
use typst::syntax::Source;
use typst::util::Bytes;
use typst::World;

use crate::workspace::project::Project;

use super::clock::Now;

/// Short-lived struct to implement [`World`] for [`Project`]. It wraps a `Project` with a main file
/// and exists for the lifetime of a Typst invocation.
#[derive(Debug)]
pub struct ProjectWorld {
    project: Project,
    main: FileId,
    /// Current time. Will be cached lazily for consistency throughout a compilation.
    now: Now,
}

impl ProjectWorld {
    pub fn new(project: Project, main: FileId) -> Self {
        Self {
            project,
            main,
            now: Now::new(),
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }
}

impl World for ProjectWorld {
    fn library(&self) -> &Prehashed<Library> {
        &self.project().workspace().typst_stdlib
    }

    fn book(&self) -> &Prehashed<FontBook> {
        self.project().workspace().font_manager().book()
    }

    fn main(&self) -> Source {
        let handle_no_main = |err| {
            error!(
                ?err,
                "this is a bug: failed to get main with id {}", self.main
            );
            warn!("returning fake main file");
            Source::detached("")
        };

        self.source(self.main).unwrap_or_else(handle_no_main)
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.project().read_source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.project().read_bytes(id)
    }

    fn font(&self, id: usize) -> Option<Font> {
        self.project().workspace().font_manager().font(id)
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        self.now.with_typst_offset(offset)
    }

    fn packages(&self) -> &[(PackageSpec, Option<EcoString>)] {
        // TODO: implement packages
        &[]
    }
}
