use anyhow::Context;
use tower_lsp::lsp_types::Url;
use tracing::info;
use typst::doc::Document;

use crate::ext::UrlExt;

use super::TypstServer;

impl TypstServer {
    #[tracing::instrument(skip(self))]
    pub async fn export_pdf(&self, source_uri: &Url, document: Document) -> anyhow::Result<()> {
        let data = self.thread(move |_| typst::export::pdf(&document)).await;
        let pdf_uri = source_uri.clone().with_extension("pdf")?;
        info!(%pdf_uri, "exporting PDF");

        let thread_uri = pdf_uri.clone();
        self.thread_with_world(&pdf_uri)
            .await?
            .run(move |world| {
                world
                    .write_raw(&thread_uri, &data)
                    .context("failed to export PDF")
            })
            .await?;

        info!("PDF export complete");

        Ok(())
    }
}
