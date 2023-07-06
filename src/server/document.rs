use tower_lsp::lsp_types::TextDocumentContentChangeEvent;
use typst::syntax::Source;

use crate::config::{Config, ExportPdfMode};
use crate::lsp_typst_boundary::world::WorkspaceWorld;
use crate::lsp_typst_boundary::LspRange;

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
            Some(lsp_range) => {
                let range = LspRange::new(lsp_range, self.get_const_config().position_encoding)
                    .to_range_on(source);
                source.edit(range, &replacement);
            }
            None => source.replace(replacement),
        }
    }

    pub async fn on_source_changed(
        &self,
        world: &WorkspaceWorld,
        config: &Config,
        source: &Source,
    ) {
        match config.export_pdf {
            ExportPdfMode::OnType => self.run_diagnostics_and_export(world, source).await,
            _ => self.run_diagnostics(world, source).await,
        }
    }

    pub async fn run_export(&self, world: &WorkspaceWorld, source: &Source) {
        let (document, _) = self.compile_source(world);

        if let Some(document) = document {
            self.export_pdf(source, &document).await;
        }
    }

    pub async fn run_diagnostics_and_export(&self, world: &WorkspaceWorld, source: &Source) {
        let (document, diagnostics) = self.compile_source(world);

        self.update_all_diagnostics(world.get_workspace(), diagnostics)
            .await;
        if let Some(document) = document {
            self.export_pdf(source, &document).await;
        }
    }

    pub async fn run_diagnostics(&self, world: &WorkspaceWorld, source: &Source) {
        let (_, diagnostics) = self.eval_source(world, source);

        self.update_all_diagnostics(world.get_workspace(), diagnostics)
            .await;
    }
}
