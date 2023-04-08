use std::collections::HashMap;

use futures::future::join_all;
use tower_lsp::lsp_types::Url;

use crate::lsp_typst_boundary::LspDiagnostic;
use crate::workspace::Workspace;

use super::TypstServer;

impl TypstServer {
    pub async fn update_all_diagnostics(
        &self,
        workspace: &Workspace,
        mut diagnostics: HashMap<Url, Vec<LspDiagnostic>>,
    ) {
        // Clear the previous diagnostics (could be done with the refresh notification when implemented by tower-lsp)
        for uri in workspace.sources.uri_iter() {
            diagnostics.entry(uri.clone()).or_insert_with(Vec::new);
        }

        let diagnostic_futures = diagnostics.into_iter().map(|(url, file_diagnostics)| {
            self.client.publish_diagnostics(url, file_diagnostics, None)
        });
        join_all(diagnostic_futures).await;
    }
}
