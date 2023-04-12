use serde_json::Value as JsonValue;
use tower_lsp::lsp_types::*;
use tower_lsp::{jsonrpc, LanguageServer};
use typst::ide::autocomplete;

use crate::config::{ConstConfig, ExportPdfMode, PositionEncoding};
use crate::ext::InitializeParamsExt;
use crate::lsp_typst_boundary::{lsp_to_typst, typst_to_lsp, TypstPathOwned};

use super::command::LspCommand;
use super::TypstServer;

#[tower_lsp::async_trait]
impl LanguageServer for TypstServer {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        // Check if a folder is opened, if yes, use it as the root path
        let root_path = match &params.root_uri {
            Some(root) => root.to_file_path().unwrap(),
            None => TypstPathOwned::new(),
        };

        let position_encoding = if params
            .position_encodings()
            .contains(&PositionEncodingKind::UTF8)
        {
            PositionEncoding::Utf8
        } else {
            PositionEncoding::Utf16
        };

        self.const_config
            .set(ConstConfig { position_encoding })
            .expect("const config should not yet be initialized");

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
                    TextDocumentSyncKind::INCREMENTAL,
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

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        let mut workspace = self.workspace.write().await;
        workspace.sources.insert(&uri, text);

        let workspace = workspace.downgrade();
        let config = self.config.read().await;

        let source_id = workspace
            .sources
            .get_id_by_uri(&uri)
            .expect("source should exist just after adding it");

        drop(workspace);

        let world = self.get_world_with_main(source_id).await;
        let source = world.get_workspace().sources.get_source_by_id(source_id);
        self.on_source_changed(&world, &config, source).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let changes = params.content_changes;

        let mut workspace = self.workspace.write().await;
        let source_id = workspace
            .sources
            .get_id_by_uri(&uri)
            .expect("source should exist after being changed");

        let source = workspace.sources.get_mut_source_by_id(source_id);
        for change in changes {
            self.apply_single_document_change(source, change);
        }

        drop(workspace);

        let world = self.get_world_with_main(source_id).await;
        let config = self.config.read().await;

        let source = world.get_workspace().sources.get_source_by_id(source_id);

        self.on_source_changed(&world, &config, source).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        let (world, source_id) = self.get_world_with_main_uri(&uri).await;
        let config = self.config.read().await;

        let source = world.get_workspace().sources.get_source_by_id(source_id);

        if config.export_pdf == ExportPdfMode::OnSave {
            self.run_diagnostics_and_export(&world, source).await;
        }
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> jsonrpc::Result<Option<JsonValue>> {
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
                return Err(jsonrpc::Error::method_not_found());
            }
        };
        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> jsonrpc::Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let (world, source_id) = self.get_world_with_main_uri(uri).await;
        let source = world.get_workspace().sources.get_source_by_id(source_id);

        Ok(self.get_hover(&world, source, position))
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let explicit = params
            .context
            .map(|context| context.trigger_kind == CompletionTriggerKind::INVOKED)
            .unwrap_or(false);

        let (world, source_id) = self.get_world_with_main_uri(uri).await;

        let source = world.get_workspace().sources.get_source_by_id(source_id);

        let typst_offset = lsp_to_typst::position_to_offset(
            position,
            self.get_const_config().position_encoding,
            source,
        );

        let completions = autocomplete(&world, &[], source.as_ref(), typst_offset, explicit);

        match completions {
            Some((_, c)) => {
                let lsp_completions = c.iter().map(typst_to_lsp::completion).collect();
                return Ok(Some(CompletionResponse::Array(lsp_completions)));
            }
            None => {
                return Ok(None);
            }
        }
    }

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> jsonrpc::Result<Option<SignatureHelp>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let (world, source_id) = self.get_world_with_main_uri(uri).await;

        let source = world.get_workspace().sources.get_source_by_id(source_id);

        Ok(self.get_signature_at_position(&world, source, position))
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        let settings = params.settings;
        let mut config = self.config.write().await;
        if let JsonValue::Object(settings) = settings {
            let export_pdf = settings
                .get("exportPdf")
                .map(|val| match val {
                    JsonValue::String(val) => match val.as_str() {
                        "never" => ExportPdfMode::Never,
                        "onSave" => ExportPdfMode::OnSave,
                        "onType" => ExportPdfMode::OnType,
                        _ => ExportPdfMode::OnSave,
                    },
                    _ => ExportPdfMode::OnSave,
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
