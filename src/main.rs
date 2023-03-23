use std::fs;
use std::path::Path;
use std::sync::Arc;

use system_world::SystemWorld;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

mod system_world;

struct Backend {
    client: Client,
    world: Arc<RwLock<Option<SystemWorld>>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let mut world = self.world.write().await;
        *world = Some(SystemWorld::new(
            params.root_uri.unwrap().to_file_path().unwrap(),
            String::new(),
        ));

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions::default()),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut world = self.world.write().await;
        let world = world.as_mut().unwrap();

        world.reset();

        match world.resolve_with(
            Path::new(&params.text_document.uri.path()),
            &params.content_changes[0].text,
        ) {
            Ok(id) => {
                world.main = id;
            }
            Err(e) => {
                self.client
                    .log_message(MessageType::ERROR, format!("{:?}", e))
                    .await;
                return;
            }
        }

        let output_path = params
            .text_document
            .uri
            .to_file_path()
            .unwrap()
            .with_extension("pdf");

        let output = match typst::compile(world) {
            Ok(document) => {
                let buffer = typst::export::pdf(&document);
                fs::write(output_path, buffer).map_err(|_| "failed to write PDF file".to_string())
            }
            Err(errors) => {
                let messages: Vec<_> = errors.iter().map(|error| error.message.as_str()).collect();
                Err(messages.join("\n"))
            }
        };

        self.client
            .log_message(MessageType::INFO, format!("{:?}", output))
            .await;
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(Some(CompletionResponse::Array(vec![
            CompletionItem::new_simple("Hello".to_string(), "Some detail".to_string()),
            CompletionItem::new_simple("Bye".to_string(), "More detail".to_string()),
        ])))
    }

    async fn hover(&self, _: HoverParams) -> Result<Option<Hover>> {
        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String("You're hovering!".to_string())),
            range: None,
        }))
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        world: Arc::new(RwLock::new(None)),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
