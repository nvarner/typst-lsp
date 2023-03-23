use std::fs;
use std::sync::Arc;

use regex::{Captures, Regex};
use system_world::SystemWorld;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use typst::World;
use typst::doc::Frame;
use typst::ide::autocomplete;
use typst::ide::CompletionKind::*;
use typst_library::prelude::EcoString;

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
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![String::from("#"), String::from("."), String::from("@")]),
                    ..Default::default()
                }),
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
        let text = &params.content_changes[0].text;
        world.update_main_source(text.clone());

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

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let position = params.text_document_position.position;
        let world = self.world.read().await;
        let source = world.as_ref().unwrap().main();

        let cursor = source.line_column_to_byte(
            position.line as _,
            position.character as _,
        ).unwrap();

        let frames: [Frame; 0] = [];

        let completions = autocomplete(world.as_ref().unwrap(), &frames, source, cursor, false);

        match completions {
            Some((_, c)) => {
                let lsp_completions = c.iter()
                    .map(completion_to_lsp_completion)
                    .collect();
                return Ok(Some(CompletionResponse::Array(lsp_completions)));
            },
            None => {
                return Ok(None);
            }
        }
    }

    async fn hover(&self, _: HoverParams) -> Result<Option<Hover>> {
        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String("You're hovering!".to_string())),
            range: None,
        }))
    }
}

/// Turn a `typst::ide::Completion` into a `lsp_types::CompletionItem`
fn completion_to_lsp_completion(completion: &typst::ide::Completion) -> CompletionItem {
    CompletionItem {
        label: completion.label.to_string(),
        kind: match completion.kind {
            Syntax => Some(CompletionItemKind::SNIPPET),
            Func => Some(CompletionItemKind::FUNCTION),
            Param => Some(CompletionItemKind::VARIABLE),
            Constant => Some(CompletionItemKind::CONSTANT),
            Symbol(_) => Some(CompletionItemKind::TEXT),
        },
        detail: completion.detail.as_ref().map(String::from),
        insert_text: completion.apply.as_ref().map(lsp_snippet),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    }
}

/// Add numbering to placeholders in snippets
fn lsp_snippet(snippet: &EcoString) -> String {
    let re = Regex::new(r"\$\{(.*?)\}").unwrap();
    let mut counter = 1;
    let result = re.replace_all(snippet.as_str(), |cap: &Captures| {
        let substitution = format!("${{{}:{}}}", counter, &cap[1]);
        counter += 1;
        substitution
    });

    result.to_string()
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
