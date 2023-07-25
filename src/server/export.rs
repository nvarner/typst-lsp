use tracing::{error, info};
use typst::doc::Document;
use typst::syntax::Source;

use crate::ext::FileIdExt;
use crate::lsp_typst_boundary::world::ProjectWorld;

use super::TypstServer;

impl TypstServer {
    pub fn export_pdf(&self, world: &ProjectWorld, source: &Source, document: &Document) {
        let output_id = source.id().with_extension("pdf");
        let data = typst::export::pdf(document);

        let result = world.project().write_raw(output_id, &data);
        match result {
            Ok(_) => {
                info!(%output_id, "exported PDF");
            }
            Err(err) => {
                error!(%err, "failed to export PDF");
            }
        };
    }
}
