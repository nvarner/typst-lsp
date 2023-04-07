use comemo::Prehashed;
use typst::diag::{FileError, FileResult};
use typst::eval::Library;
use typst::font::{Font, FontBook};
use typst::util::Buffer;
use typst::World;

use crate::workspace::Workspace;

use super::{typst_to_lsp, TypstPath, TypstSource, TypstSourceId};

impl World for Workspace {
    fn library(&self) -> &Prehashed<Library> {
        &self.typst_stdlib
    }

    fn main(&self) -> &TypstSource {
        // The best `main` file depends on what the LSP is doing. For example, when providing
        // diagnostics, the file for which diagnostics are being produced is the best choice of
        // `main`. However, that means `main` needs to change between invocations of Typst
        // functions, but stay constant across each of them. This is very hard to do with the
        // `'static` requirement from `comemo`.
        //
        // The most obvious way would to store the current `main` in `Workspace`, setting it each
        // time we call a Typst function and using a synchronization object to maintain it. However,
        // this becomes difficult, and leads to storing state local to a single function call within
        // global `Workspace` state, which is a bad idea.
        //
        // Ideally, we would instead implement `World` for something like `(&Workspace, SourceId)`,
        // so that each caller who wants to use `Workspace` as a `World` must declare what `main`
        // should be via a `SourceId`. However, the `'static` requirement prevents this, and
        // `(Workspace, SourceId)` or even `(Rc<Workspace>, SourceId)` would increase complexity
        // substantially.
        //
        // So in order of theoretical niceness, the best solutions are:
        // - Relax the `'static` requirement from `comemo` (if that is even possible)
        // - Fork `typst` just to remove `main`, leading to tons of extra work
        // - Disallow calling `main` on `Workspace`
        //
        // To be clear, this is also a bad idea. However, at time of writing, `main` seems to be
        // called in only two places in the `typst` library (`compile` and `analyze_expr`), both of
        // which can be worked around as needed. Assuming this holds true into the future,
        // invocations of `main` should be easy to catch and avoid during development, so this is
        // good enough.
        panic!("should not invoke `World`'s `main` on a `Workspace` because there is no reasonable default context")
        // tokio::task::block_in_place(|| {
        //     let lsp_source = self
        //         .sources
        //         .get_source_by_id(self.main_id.blocking_read().unwrap());
        //     lsp_source.as_ref()
        // })
    }

    fn resolve(&self, typst_path: &TypstPath) -> FileResult<TypstSourceId> {
        let lsp_uri = typst_to_lsp::path_to_uri(typst_path).unwrap();
        let lsp_id = self.sources.get_id_by_uri(&lsp_uri);
        match lsp_id {
            Some(lsp_id) => Ok(lsp_id.into()),
            None => Err(FileError::NotFound(typst_path.to_owned())),
        }
    }

    fn source(&self, typst_id: TypstSourceId) -> &TypstSource {
        let lsp_source = self.sources.get_source_by_id(typst_id.into());
        lsp_source.as_ref()
    }

    fn book(&self) -> &Prehashed<FontBook> {
        self.fonts.book()
    }

    fn font(&self, id: usize) -> Option<Font> {
        let mut resources = self.resources.write();
        self.fonts.font(id, &mut resources)
    }

    fn file(&self, typst_path: &TypstPath) -> FileResult<Buffer> {
        let lsp_uri = typst_to_lsp::path_to_uri(typst_path).unwrap();
        let mut resources = self.resources.write();
        let lsp_resource = resources.get_or_insert_resource(lsp_uri)?;
        Ok(lsp_resource.into())
    }
}
