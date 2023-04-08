use std::fs;

use tower_lsp::lsp_types::MessageType;
use typst::doc::Document;

use crate::workspace::source::Source;
use crate::workspace::Workspace;

use super::log::LogMessage;
use super::TypstServer;

impl TypstServer {
    pub async fn export_pdf(&self, _workspace: &Workspace, source: &Source, document: &Document) {
        let buffer = typst::export::pdf(document);
        let output_path = source.as_ref().path().with_extension("pdf");

        let result = fs::write(&output_path, buffer);

        match result {
            Ok(_) => {
                let message = LogMessage {
                    message_type: MessageType::INFO,
                    message: format!("File written to {}", output_path.to_string_lossy()),
                };
                self.log_to_client(message).await;
            }
            Err(e) => {
                let message = LogMessage {
                    message_type: MessageType::ERROR,
                    message: e.to_string(),
                };
                self.log_to_client(message).await;
            }
        };
    }
}
