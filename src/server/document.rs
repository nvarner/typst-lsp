use anyhow::bail;
use tower_lsp::lsp_types::Url;

use crate::config::ExportPdfMode;

use super::TypstServer;

impl TypstServer {
    pub async fn on_source_changed(&self, uri: &Url) -> anyhow::Result<()> {
        let config = self.config.read().await;
        match config.export_pdf {
            ExportPdfMode::OnType => self.run_diagnostics_and_export(uri).await?,
            _ => self.run_diagnostics(uri).await?,
        }

        Ok(())
    }

    pub async fn run_export(&self, uri: &Url) -> anyhow::Result<()> {
        let (Some(document), _) = self.compile_source(uri).await? else {
            bail!("failed to generate document after compilation")
        };

        self.export_pdf(uri, document).await?;

        Ok(())
    }

    pub async fn run_diagnostics_and_export(&self, uri: &Url) -> anyhow::Result<()> {
        let (document, diagnostics) = self.compile_source(uri).await?;

        self.update_all_diagnostics(diagnostics).await;
        if let Some(document) = document {
            self.export_pdf(uri, document).await?;
        } else {
            bail!("failed to generate document after compilation")
        }

        Ok(())
    }

    pub async fn run_diagnostics(&self, uri: &Url) -> anyhow::Result<()> {
        let (_, diagnostics) = self.compile_source(uri).await?;

        self.update_all_diagnostics(diagnostics).await;

        Ok(())
    }
}
