use anyhow::Context;
use tracing::info;
use typst::doc::Document;
use typst::syntax::Source;

use crate::ext::FileIdExt;
use crate::lsp_typst_boundary::world::ProjectWorld;

use super::TypstServer;

impl TypstServer {
    #[tracing::instrument(skip(self))]
    pub async fn export_pdf(
        &self,
        world: &ProjectWorld,
        source: &Source,
        document: &Document,
    ) -> anyhow::Result<()> {
        let data = typst::export::pdf(document);

        let id = source.id().with_extension("pdf");
        let full_id = world.fill_id(id);
        let uri = world.full_id_to_uri(full_id).await?;

        world
            .workspace()
            .write_raw(&uri, &data)
            .context("failed to export PDF")?;

        info!(%id, "exported PDF");

        Ok(())
    }
}
