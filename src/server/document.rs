use tower_lsp::lsp_types::TextDocumentContentChangeEvent;

use crate::config::{Config, ExportPdfMode};
use crate::lsp_typst_boundary::LspRange;
use crate::workspace::source::Source;
use crate::workspace::Workspace;

use super::TypstServer;

impl TypstServer {
    /// Apply a single change event to a document
    pub fn apply_single_document_change(
        &self,
        source: &mut Source,
        change: TextDocumentContentChangeEvent,
    ) {
        let replacement = change.text;

        match change.range {
            Some(range) => {
                let range = LspRange::new(range, self.get_const_config().position_encoding);
                source.edit(&range, &replacement);
            }
            None => source.replace(replacement),
        }
    }

    pub async fn on_source_changed(&self, workspace: &Workspace, config: &Config, source: &Source) {
        match config.export_pdf {
            ExportPdfMode::OnType => self.run_diagnostics_and_export(workspace, source).await,
            _ => self.run_diagnostics(workspace, source).await,
        }
    }

    pub async fn run_export(&self, workspace: &Workspace, source: &Source) {
        let (document, _) = self.compile_source(workspace, source);

        if let Some(document) = document {
            self.export_pdf(workspace, source, &document).await;
        }
    }

    pub async fn run_diagnostics_and_export(&self, workspace: &Workspace, source: &Source) {
        let (document, diagnostics) = self.compile_source(workspace, source);

        self.update_all_diagnostics(workspace, diagnostics).await;
        if let Some(document) = document {
            self.export_pdf(workspace, source, &document).await;
        }
    }

    pub async fn run_diagnostics(&self, workspace: &Workspace, source: &Source) {
        let (_, diagnostics) = self.eval_source(workspace, source);

        self.update_all_diagnostics(workspace, diagnostics).await;
    }
}
