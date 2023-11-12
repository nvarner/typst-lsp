use std::sync::Arc;

use anyhow::Context;
use tower_lsp::lsp_types::Url;
use tracing::info;
use typst::doc::Document;

use crate::ext::UrlExt;

use super::TypstServer;

impl TypstServer {
    #[tracing::instrument(skip(self))]
    pub async fn export_pdf(
        &self,
        source_uri: &Url,
        document: Arc<Document>,
    ) -> anyhow::Result<()> {
        let pdf_uri = source_uri.clone().with_extension("pdf")?;
        info!(%pdf_uri, "exporting PDF");

        self.thread_with_world(source_uri)
            .await?
            .run(move |world| {
                let data = typst::export::pdf(&document, Some(pdf_uri.as_str()), world.now());

                world
                    .write_raw(&pdf_uri, &data)
                    .context("failed to export PDF")
            })
            .await?;

        info!("PDF export complete");

        Ok(())
    }
}
