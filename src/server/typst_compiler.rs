use comemo::Track;
use typst::doc::Document;
use typst::eval::{Module, Route, Tracer};
use typst::World;

use crate::lsp_typst_boundary::workaround::compile;
use crate::lsp_typst_boundary::{typst_to_lsp, LspDiagnostics};
use crate::workspace::source::Source;
use crate::workspace::Workspace;

use super::TypstServer;

impl TypstServer {
    pub fn compile_source(
        &self,
        workspace: &Workspace,
        source: &Source,
    ) -> (Option<Document>, LspDiagnostics) {
        let result = compile(workspace, source.as_ref());

        let (document, errors) = match result {
            Ok(document) => (Some(document), Default::default()),
            Err(errors) => (Default::default(), errors),
        };

        let diagnostics = typst_to_lsp::source_errors_to_diagnostics(
            errors.as_ref(),
            workspace,
            self.get_const_config(),
        );

        // Garbage collect incremental cache. This evicts all memoized results that haven't been
        // used in the last 30 compilations.
        comemo::evict(30);

        (document, diagnostics)
    }

    pub fn eval_source(
        &self,
        workspace: &Workspace,
        source: &Source,
    ) -> (Option<Module>, LspDiagnostics) {
        let route = Route::default();
        let mut tracer = Tracer::default();
        let result = typst::eval::eval(
            (workspace as &dyn World).track(),
            route.track(),
            tracer.track_mut(),
            source.as_ref(),
        );

        let (module, errors) = match result {
            Ok(module) => (Some(module), Default::default()),
            Err(errors) => (Default::default(), errors),
        };

        let diagnostics = typst_to_lsp::source_errors_to_diagnostics(
            errors.as_ref(),
            workspace,
            self.get_const_config(),
        );

        // Garbage collect incremental cache. This evicts all memoized results that haven't been
        // used in the last 30 compilations.
        comemo::evict(30);

        (module, diagnostics)
    }
}
