use std::fmt::Display;

use tokio::runtime::Handle;
use tower_lsp::lsp_types::MessageType;
use tower_lsp::Client;
use tracing::metadata::LevelFilter;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

use super::TypstServer;

// Message that is sent to the client
#[derive(Debug, Clone)]
pub struct LogMessage<M: Display> {
    pub message_type: MessageType,
    pub message: M,
}

impl TypstServer {
    pub fn set_tracing_global_subscriber(&self) {
        tracing_subscriber::registry()
            .with(LspLayer::new(self.client.clone()).with_filter(LevelFilter::INFO))
            .init()
    }

    pub async fn log_to_client<M: Display>(&self, message: LogMessage<M>) {
        self.client
            .log_message(message.message_type, message.message)
            .await;
    }
}

pub struct LspLayer {
    client: Client,
}

impl LspLayer {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for LspLayer {
    fn on_event(&self, event: &Event, _ctx: Context<S>) {
        if let Ok(handle) = Handle::try_current() {
            let client = self.client.clone();
            let message_type = level_to_message_type(*event.metadata().level());
            let message = format!("event: {}", event.metadata().name());

            handle.spawn(async move {
                client.log_message(message_type, message).await;
            });
        }
    }
}

fn level_to_message_type(level: Level) -> MessageType {
    match level {
        Level::ERROR => MessageType::ERROR,
        Level::WARN => MessageType::WARNING,
        Level::INFO => MessageType::INFO,
        Level::DEBUG | Level::TRACE => MessageType::LOG,
    }
}
