use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;
use system_world::SystemWorld;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use typst::diag::SourceError;
use typst::syntax::Source;
use typst::World;

mod config;
mod system_world;

struct Backend {
    client: Client,
    world: Arc<RwLock<Option<SystemWorld>>>,
    config: Arc<RwLock<config::Config>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let mut world = self.world.write().await;
        *world = Some(SystemWorld::new(
            params.root_uri.unwrap().to_file_path().unwrap(),
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

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.on_change(params.text_document.uri, params.text_document.text)
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        let text = params.content_changes.pop().unwrap().text;
        self.on_change(params.text_document.uri, text).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let text = params.text.unwrap_or_else(|| {
            fs::read_to_string(
                params
                    .text_document
                    .uri
                    .to_file_path()
                    .expect("Could not convert URI to file path"),
            )
            .expect("Could not read file")
        });
        self.on_save(params.text_document.uri, text).await;
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

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        let settings = params.settings;
        let mut config = self.config.write().await;
        if let Value::Object(settings) = settings {
            let export_pdf = settings
                .get("exportPdf")
                .map(|val| match val {
                    Value::String(val) => match val.as_str() {
                        "never" => config::ExportPdfMode::Never,
                        "onSave" => config::ExportPdfMode::OnSave,
                        "onType" => config::ExportPdfMode::OnType,
                        _ => config::ExportPdfMode::OnSave,
                    },
                    _ => config::ExportPdfMode::OnSave,
                })
                .unwrap_or_default();
            config.export_pdf = export_pdf;
            self.client
                .log_message(MessageType::INFO, "New settings applied")
                .await;
        } else {
            self.client
                .log_message(MessageType::ERROR, "Got invalid configuration object")
                .await;
        }
    }
}

impl Backend {
    async fn on_change(&self, uri: Url, text: String) {
        let config = self.config.read().await;
        self.compile_diags_export(
            uri,
            text,
            matches!(config.export_pdf, config::ExportPdfMode::OnType),
        )
        .await;
    }

    async fn on_save(&self, uri: Url, text: String) {
        let config = self.config.read().await;
        self.compile_diags_export(
            uri,
            text,
            matches!(config.export_pdf, config::ExportPdfMode::OnSave),
        )
        .await;
    }

    async fn compile_diags_export(&self, uri: Url, text: String, export: bool) {
        let mut world_lock = self.world.write().await;
        let world = world_lock.as_mut().unwrap();

        world.reset();

        match world.resolve_with(Path::new(&uri.to_file_path().unwrap()), &text) {
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

        let mut fs_message: Option<LogMessage> = None; // log success or error of file write
        let messages: Vec<_> = match typst::compile(world) {
            Ok(document) => {
                let buffer = typst::export::pdf(&document);
                if export {
                    let output_path = uri.to_file_path().unwrap().with_extension("pdf");
                    fs_message = match fs::write(&output_path, buffer)
                        .map_err(|_| "failed to write PDF file".to_string())
                    {
                        Ok(_) => Some(LogMessage {
                            message_type: MessageType::INFO,
                            message: format!("File written to {}", output_path.to_string_lossy()),
                        }),
                        Err(e) => Some(LogMessage {
                            message_type: MessageType::ERROR,
                            message: format!("{:?}", e),
                        }),
                    };
                }
                vec![]
            }
            Err(errors) => errors.iter().map(|x| error_to_range(x, world)).collect(),
        };
        // release the lock early
        drop(world_lock);

        // we can't await while we hold a lock on the world so we do it now
        if let Some(msg) = fs_message {
            self.client.log_message(msg.message_type, msg.message).await;
        }

        self.client
            .publish_diagnostics(
                uri.clone(),
                messages
                    .into_iter()
                    .map(|(message, range)| Diagnostic {
                        range,
                        severity: Some(DiagnosticSeverity::ERROR),
                        message,
                        ..Default::default()
                    })
                    .collect(),
                None,
            )
            .await;
    }
}

// Message that is send to the client
pub struct LogMessage {
    pub message_type: MessageType,
    pub message: String,
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        world: Arc::new(RwLock::new(None)),
        config: Arc::new(RwLock::new(config::Config::default())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

fn error_to_range(error: &SourceError, world: &SystemWorld) -> (String, Range) {
    let source = world.source(error.span.source());
    let range = source.range(error.span);
    let range = range_to_lsp_range(range, source);
    (error.message.to_string(), range)
}

fn range_to_lsp_range(range: std::ops::Range<usize>, source: &Source) -> Range {
    Range {
        start: Position {
            line: source.byte_to_line(range.start).unwrap() as _,
            character: source.byte_to_column(range.start).unwrap() as _,
        },
        end: Position {
            line: source.byte_to_line(range.end).unwrap() as _,
            character: source.byte_to_column(range.end).unwrap() as _,
        },
    }
}
