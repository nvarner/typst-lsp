use anyhow::Context;
use tower_lsp::lsp_types::Url;
use tracing::info;
use typst::doc::Document;

use super::TypstServer;

impl TypstServer {
    #[tracing::instrument(skip(self))]
    pub async fn export_pdf(&self, source_uri: &Url, document: Document) -> anyhow::Result<()> {
        let data = self.thread(move |_| typst::export::pdf(&document)).await;

        let (project, full_id) = self.project_and_full_id(source_uri).await?;
        let pdf_full_id = full_id.with_extension("pdf");
        let pdf_uri = project.full_id_to_uri(pdf_full_id).await?;

        let thread_uri = pdf_uri.clone();
        self.thread_with_world((pdf_full_id, &pdf_uri))
            .await?
            .run(move |world| {
                world
                    .write_raw(&thread_uri, &data)
                    .context("failed to export PDF")
            })
            .await?;

        info!(id = ?pdf_full_id, "exported PDF");

        Ok(())
    }
}
