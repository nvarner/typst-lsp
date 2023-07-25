use typst::syntax::Source;

use crate::config::{Config, ExportPdfMode};
use crate::lsp_typst_boundary::world::ProjectWorld;

use super::TypstServer;

impl TypstServer {
    pub async fn on_source_changed(&self, world: &ProjectWorld, config: &Config, source: &Source) {
        match config.export_pdf {
            ExportPdfMode::OnType => self.run_diagnostics_and_export(world, source).await,
            _ => self.run_diagnostics(world, source).await,
        }
    }

    pub async fn run_export(&self, world: &ProjectWorld, source: &Source) {
        let (document, _) = self.compile_source(world);

        if let Some(document) = document {
            self.export_pdf(source, &document).await;
        }
    }

    pub async fn run_diagnostics_and_export(&self, world: &ProjectWorld, source: &Source) {
        let (document, diagnostics) = self.compile_source(world);

        self.update_all_diagnostics(diagnostics).await;
        if let Some(document) = document {
            self.export_pdf(source, &document).await;
        }
    }

    pub async fn run_diagnostics(&self, world: &ProjectWorld, source: &Source) {
        let (_, diagnostics) = self.eval_source(world, source);

        self.update_all_diagnostics(diagnostics).await;
    }
}
