use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use regex::{Captures, Regex};
use serde_json::Value as JsonValue;
use system_world::SystemWorld;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::{Error, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use typst::diag::SourceError;
use typst::doc::Frame;
use typst::eval::{CastInfo, FuncInfo, Value};
use typst::ide::autocomplete;
use typst::ide::CompletionKind::*;
use typst::ide::{tooltip, Tooltip};
use typst::syntax::{ast, LinkedNode, Source, SyntaxKind};
use typst::World;
use typst_library::prelude::EcoString;

use crate::command::LspCommand;

mod command;
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
        // Check if a folder is opened, if yes, use it as the root path
        let root_path = if let Some(root) = params.root_uri {
            root.to_file_path().unwrap()
        } else {
            PathBuf::new()
        };
        *world = Some(SystemWorld::new(root_path));
        
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        String::from("#"),
                        String::from("."),
                        String::from("@"),
                    ]),
                    ..Default::default()
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: LspCommand::all_as_string(),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
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

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<JsonValue>> {
        let ExecuteCommandParams {
            command,
            arguments,
            work_done_progress_params: _,
        } = params;
        self.client.log_message(MessageType::INFO, &command).await;
        match LspCommand::parse(&command) {
            Some(LspCommand::ExportPdf) => {
                self.command_export_pdf(arguments).await?;
            }
            None => {
                return Err(Error::method_not_found());
            }
        };
        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let world = self.world.read().await;
        let world = world.as_ref().unwrap();
        let source = world.main();

        let Some(cursor) = get_cursor_for_position(params.text_document_position_params.position, source) else {return Ok(None)};

        let Some(tooltip) = tooltip(world, &[], source, cursor) else {return Ok(None)};
        let tooltip = match tooltip {
            Tooltip::Text(s) => s,
            Tooltip::Code(s) => s,
        };

        let Some(lk) = LinkedNode::new(source.root()).leaf_at(cursor) else {return Ok(None)};

        let range = range_to_lsp_range(lk.range(), source);
        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(tooltip.into())),
            range: Some(range),
        }))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let position = params.text_document_position.position;
        let world = self.world.read().await;
        let source = world.as_ref().unwrap().main();

        let cursor = source
            .line_column_to_byte(position.line as _, position.character as _)
            .unwrap();

        let frames: [Frame; 0] = [];

        let completions = autocomplete(world.as_ref().unwrap(), &frames, source, cursor, false);

        match completions {
            Some((_, c)) => {
                let lsp_completions = c.iter().map(completion_to_lsp_completion).collect();
                return Ok(Some(CompletionResponse::Array(lsp_completions)));
            }
            None => {
                return Ok(None);
            }
        }
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        let settings = params.settings;
        let mut config = self.config.write().await;
        if let JsonValue::Object(settings) = settings {
            let export_pdf = settings
                .get("exportPdf")
                .map(|val| match val {
                    JsonValue::String(val) => match val.as_str() {
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

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let help = self
            .signature_help(
                params.text_document_position_params.text_document.uri,
                params.text_document_position_params.position,
            )
            .await;

        Ok(help)
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

        let mut fs_message: Option<LogMessage<String>> = None; // log success or error of file write
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

    async fn signature_help(&self, _uri: Url, position: Position) -> Option<SignatureHelp> {
        let world = self.world.read().await;
        let world = world.as_ref().unwrap();
        let global = world.library().global.scope();
        let source = world.main();

        let cursor = source.line_column_to_byte(position.line as _, position.character as _)?;

        let leaf = LinkedNode::new(source.root()).leaf_at(cursor)?;
        let parent = leaf.parent()?;
        let parent = match parent.kind() {
            SyntaxKind::Named => parent.parent()?,
            _ => parent,
        };
        let args = parent.cast::<ast::Args>()?;
        let grand = parent.parent()?;
        let expr = grand.cast::<ast::Expr>()?;
        let callee = match expr {
            ast::Expr::FuncCall(call) => call.callee(),
            ast::Expr::Set(set) => set.target(),
            _ => return None,
        };
        let callee = match callee {
            ast::Expr::Ident(callee) => callee,
            _ => return None,
        };

        // Find the piece of syntax that decides what we're completing.
        let mut deciding = leaf.clone();
        while !matches!(
            deciding.kind(),
            SyntaxKind::LeftParen | SyntaxKind::Comma | SyntaxKind::Colon
        ) {
            let Some(prev) = deciding.prev_leaf() else { break };
            deciding = prev;
        }

        let Some(Value::Func(func)) = global.get(&callee) else { return None };
        let info = func.info()?;

        let mut completing_param = None;

        // After colon: "func(param:|)", "func(param: |)".
        if deciding.kind() == SyntaxKind::Colon {
            if let Some(prev) = deciding.prev_leaf() {
                if let Some(param_ident) = prev.cast::<ast::Ident>() {
                    completing_param = info
                        .params
                        .iter()
                        .position(|param| param.name == param_ident.as_str());
                }
            }
        }
        // Before: "func(|)", "func(hi|)", "func(12,|)".
        if deciding.kind() == SyntaxKind::Comma || deciding.kind() == SyntaxKind::LeftParen {
            if let Some(next) = deciding
                .next_leaf()
                .and_then(|next| next.cast::<ast::Ident>())
            {
                completing_param = info
                    .params
                    .iter()
                    .position(|param| param.named && param.name.starts_with(next.as_str()));
            } else {
                let n_positional = args
                    .items()
                    .filter(|arg| matches!(arg, ast::Arg::Pos(_)))
                    .count();
                completing_param = info
                    .params
                    .iter()
                    .enumerate()
                    .filter(|(_, param)| param.positional)
                    .map(|(i, _)| i)
                    .nth(n_positional);
            }
        }

        let (label, params) = parameter_information(info, completing_param);

        let help = SignatureHelp {
            signatures: vec![SignatureInformation {
                label,
                documentation: Some(markdown_docs(info.docs)),
                parameters: Some(params),
                active_parameter: completing_param.map(|i| i as u32),
            }],
            active_signature: Some(0),
            active_parameter: None,
        };

        Some(help)
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

/// Returns the signature label as well as parameter offsets of the function
fn parameter_information(
    info: &FuncInfo,
    completing_param: Option<usize>,
) -> (String, Vec<ParameterInformation>) {
    let mut params = Vec::new();
    let mut label = info.name.to_owned();
    label.push('(');
    let mut first = true;

    for (i, param) in info.params.iter().enumerate() {
        if !first {
            label.push_str(", ");
        }
        first = false;

        let start = label.chars().count();

        label.push_str(param.name);
        let include_type = Some(i) == completing_param;
        if include_type {
            label.push_str(": ");
            format_cast_info(&mut label, &param.cast);
        }

        let end = label.chars().count();

        params.push(ParameterInformation {
            label: ParameterLabel::LabelOffsets([start as u32, end as u32]),
            documentation: Some(markdown_docs(param.docs)),
        });
    }
    label.push(')');
    if !info.returns.is_empty() {
        label.push_str(" -> ");
        let mut first = true;
        for &ret in &info.returns {
            if !first {
                label.push_str(", ");
            }
            first = false;
            label.push_str(ret);
        }
    }
    (label, params)
}

fn format_cast_info(s: &mut String, info: &CastInfo) {
    match info {
        CastInfo::Any => s.push_str("anything"),
        CastInfo::Value(value, _) => {
            s.push_str(&value.repr());
        }
        CastInfo::Type(ty) => s.push_str(ty),
        CastInfo::Union(options) => {
            let mut first = true;
            for option in options {
                if !first {
                    s.push(' ')
                };
                first = false;
                format_cast_info(s, option);
            }
        }
    }
}

fn markdown_docs(docs: &str) -> Documentation {
    Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: docs.into(),
    })
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

// Message that is send to the client
#[derive(Debug, Clone)]
pub struct LogMessage<M: Display> {
    pub message_type: MessageType,
    pub message: M,
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

fn get_cursor_for_position(pos: Position, source: &Source) -> Option<usize> {
    source.line_column_to_byte(pos.line as _, pos.character as _)
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
