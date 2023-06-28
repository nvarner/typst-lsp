use std::fmt::{self, Write};

use tokio::runtime::Handle;
use tower_lsp::lsp_types::MessageType;
use tower_lsp::Client;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

use super::TypstServer;

impl TypstServer {
    pub fn tracing_init(&self) {
        let lsp_layer = LspLayer::new(self.client.clone());
        self.lsp_tracing_layer_handle
            .reload(Some(lsp_layer))
            .expect("should be able to replace layer, since it should only fail when there is a larger issue with the `Subscriber`");
    }
}

pub struct LspLayer {
    client: Client,
}

impl LspLayer {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn should_skip(event: &Event) -> bool {
        // these events are emitted when logging to client, causing a recursive chain reaction
        event.metadata().target().contains("codec")
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for LspLayer {
    fn on_event<'b>(&self, event: &Event<'b>, _ctx: Context<S>) {
        if Self::should_skip(event) {
            return;
        }

        if let Ok(handle) = Handle::try_current() {
            let client = self.client.clone();
            let metadata: &Metadata<'b> = event.metadata();

            let message_type = level_to_message_type(*metadata.level());

            let line_info: (Option<&'b str>, _) = (metadata.file(), metadata.line());
            let mut message = match line_info {
                (Some(file), Some(line)) => format!("{file}:{line} {{"),
                (Some(file), None) => format!("{file} {{"),
                (None, _) => "{".to_owned(),
            };

            event.record(&mut LspVisit::with_string(&mut message));

            message.push_str(" }");

            handle.spawn(async move {
                client.log_message(message_type, message).await;
            });
        }
    }
}

struct LspVisit<'a> {
    message: &'a mut String,
}

impl<'a> LspVisit<'a> {
    pub fn with_string(string: &'a mut String) -> Self {
        Self { message: string }
    }
}

impl<'a> Visit for LspVisit<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        write!(self.message, " {} = {:?};", field.name(), value).unwrap();
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
