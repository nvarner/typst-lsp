use std::fs;

use tracing::{error, info};
use typst::doc::Document;

use crate::workspace::source::Source;

use super::TypstServer;

impl TypstServer {
    pub async fn export_pdf(&self, source: &Source, document: &Document) {
        let buffer = typst::export::pdf(document);
        let output_path = source.as_ref().path().with_extension("pdf");

        let result = fs::write(&output_path, buffer);

        match result {
            Ok(_) => {
                info!(?output_path, "exported PDF");
            }
            Err(err) => {
                error!(?err, "failed to export PDF");
            }
        };
    }
}
