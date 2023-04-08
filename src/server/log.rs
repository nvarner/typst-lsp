use std::fmt::Display;

use tower_lsp::lsp_types::MessageType;

use super::TypstServer;

// Message that is sent to the client
#[derive(Debug, Clone)]
pub struct LogMessage<M: Display> {
    pub message_type: MessageType,
    pub message: M,
}

impl TypstServer {
    pub async fn log_to_client<M: Display>(&self, message: LogMessage<M>) {
        self.client
            .log_message(message.message_type, message.message)
            .await;
    }
}
