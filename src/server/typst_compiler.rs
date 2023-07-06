use comemo::Track;
use typst::doc::Document;
use typst::eval::{Module, Route, Tracer};
use typst::syntax::Source;
use typst::World;

use crate::lsp_typst_boundary::typst_to_lsp;
use crate::lsp_typst_boundary::world::WorkspaceWorld;

use super::diagnostics::DiagnosticsMap;
use super::TypstServer;

impl TypstServer {
    pub fn compile_source(&self, world: &WorkspaceWorld) -> (Option<Document>, DiagnosticsMap) {
        let result = typst::compile(world);

        let (document, errors) = match result {
            Ok(document) => (Some(document), Default::default()),
            Err(errors) => (Default::default(), errors),
        };

        let diagnostics = typst_to_lsp::source_errors_to_diagnostics(
            errors.as_ref(),
            world,
            self.get_const_config(),
        );

        // Garbage collect incremental cache. This evicts all memoized results that haven't been
        // used in the last 30 compilations.
        comemo::evict(30);

        (document, diagnostics)
    }

    pub fn eval_source(
        &self,
        world: &WorkspaceWorld,
        source: &Source,
    ) -> (Option<Module>, DiagnosticsMap) {
        let route = Route::default();
        let mut tracer = Tracer::default();
        let result = typst::eval::eval(
            (world as &dyn World).track(),
            route.track(),
            tracer.track_mut(),
            source,
        );

        let (module, errors) = match result {
            Ok(module) => (Some(module), Default::default()),
            Err(errors) => (Default::default(), errors),
        };

        let diagnostics = typst_to_lsp::source_errors_to_diagnostics(
            errors.as_ref(),
            world,
            self.get_const_config(),
        );

        // Garbage collect incremental cache. This evicts all memoized results that haven't been
        // used in the last 30 compilations.
        comemo::evict(30);

        (module, diagnostics)
    }
}
