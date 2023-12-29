use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use futures::FutureExt;
use itertools::Itertools;
use serde_json::Value as JsonValue;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::*;
use tower_lsp::{jsonrpc, LanguageServer};
use tracing::{error, info, trace, warn};
use typst::World;

use crate::config::{
    get_config_registration, Config, ConstConfig, ExperimentalFormatterMode, ExportPdfMode,
    SemanticTokensMode,
};
use crate::ext::InitializeParamsExt;
use crate::lsp_typst_boundary::typst_to_lsp::offset_to_position;
use crate::lsp_typst_boundary::{lsp_to_typst, typst_to_lsp, LspRawRange};
use crate::server::formatting::{get_formatting_registration, get_formatting_unregistration};
use crate::workspace::Workspace;

use super::command::LspCommand;
use super::semantic_tokens::{
    get_semantic_tokens_options, get_semantic_tokens_registration,
    get_semantic_tokens_unregistration,
};
use super::TypstServer;

#[async_trait]
impl LanguageServer for TypstServer {
    #[tracing::instrument(skip(self))]
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        self.tracing_init();

        self.workspace
            .set(Arc::new(RwLock::new(Workspace::new(&params))))
            .map_err(|_| ())
            .expect("workspace should not yet be initialized");

        self.const_config
            .set(ConstConfig::from(&params))
            .expect("const config should not yet be initialized");

        if let Some(init) = &params.initialization_options {
            let mut config = self.config.write().await;
            config
                .update(init)
                .await
                .as_ref()
                .map_err(ToString::to_string)
                .map_err(jsonrpc::Error::invalid_params)?;
        }

        if let Err(err) = self.register_workspace_files().await {
            error!(%err, "could not register workspace files on init");
            return Err(jsonrpc::Error::internal_error());
        }

        let config = self.config.read().await;

        let semantic_tokens_provider = match config.semantic_tokens {
            SemanticTokensMode::Enable
                if !params.supports_semantic_tokens_dynamic_registration() =>
            {
                Some(get_semantic_tokens_options().into())
            }
            _ => None,
        };

        let document_formatting_provider = match config.formatter {
            ExperimentalFormatterMode::On
                if !params.supports_document_formatting_dynamic_registration() =>
            {
                Some(OneOf::Left(true))
            }
            _ => None,
        };

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
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..Default::default()
                    },
                )),
                semantic_tokens_provider,
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: LspCommand::all_as_string(),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    ..Default::default()
                }),
                document_formatting_provider,
                ..Default::default()
            },
            ..Default::default()
        })
    }

    #[tracing::instrument(skip_all)]
    async fn initialized(&self, _: InitializedParams) {
        let const_config = self.const_config();
        let mut config = self.config.write().await;

        if const_config.supports_semantic_tokens_dynamic_registration {
            trace!("setting up to dynamically register semantic token support");

            let client = self.client.clone();
            let register = move || {
                trace!("dynamically registering semantic tokens");
                let client = client.clone();
                async move {
                    let options = get_semantic_tokens_options();
                    client
                        .register_capability(vec![get_semantic_tokens_registration(options)])
                        .await
                        .context("could not register semantic tokens")
                }
            };

            let client = self.client.clone();
            let unregister = move || {
                trace!("unregistering semantic tokens");
                let client = client.clone();
                async move {
                    client
                        .unregister_capability(vec![get_semantic_tokens_unregistration()])
                        .await
                        .context("could not unregister semantic tokens")
                }
            };

            if config.semantic_tokens == SemanticTokensMode::Enable {
                if let Some(err) = register().await.err() {
                    error!(%err, "could not dynamically register semantic tokens");
                }
            }

            config.listen_semantic_tokens(Box::new(move |mode| match mode {
                SemanticTokensMode::Enable => register().boxed(),
                SemanticTokensMode::Disable => unregister().boxed(),
            }));
        }

        if const_config.supports_document_formatting_dynamic_registration {
            trace!("setting up to dynamically register document formatting support");

            let client = self.client.clone();
            let register = move || {
                trace!("dynamically registering document formatting");
                let client = client.clone();
                async move {
                    client
                        .register_capability(vec![get_formatting_registration()])
                        .await
                        .context("could not register document formatting")
                }
            };

            let client = self.client.clone();
            let unregister = move || {
                trace!("unregistering document formatting");
                let client = client.clone();
                async move {
                    client
                        .unregister_capability(vec![get_formatting_unregistration()])
                        .await
                        .context("could not unregister document formatting")
                }
            };

            if config.formatter == ExperimentalFormatterMode::On {
                if let Some(err) = register().await.err() {
                    error!(%err, "could not dynamically register document formatting");
                }
            }

            config.listen_formatting(Box::new(move |formatter| match formatter {
                ExperimentalFormatterMode::On => register().boxed(),
                ExperimentalFormatterMode::Off => unregister().boxed(),
            }));
        }

        if const_config.supports_config_change_registration {
            trace!("setting up to request config change notifications");

            let err = self
                .client
                .register_capability(vec![get_config_registration()])
                .await
                .err();
            if let Some(err) = err {
                error!(%err, "could not register to watch config changes");
            }
        }

        trace!("setting up to watch Typst files");
        let watch_files_error = self
            .client
            .register_capability(vec![self.get_watcher_registration()])
            .await
            .err();
        if let Some(err) = watch_files_error {
            error!(%err, "could not register to watch Typst files");
        }

        info!("server initialized");
    }

    #[tracing::instrument(skip_all)]
    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(uri = %params.text_document.uri))]
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        let mut workspace = self.workspace().write().await;

        if let Err(err) = workspace.open_lsp(uri.clone(), text) {
            error!(%err, %uri, "could not open file from LSP client");
            return;
        };

        drop(workspace);

        if let Err(err) = self.on_source_changed(&uri).await {
            error!(%err, %uri, "could not handle source change");
        };
    }

    #[tracing::instrument(skip_all, fields(uri = %params.text_document.uri))]
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;

        let mut workspace = self.workspace().write().await;

        workspace.close_lsp(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    #[tracing::instrument(skip_all, fields(uri = %params.text_document.uri))]
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let changes = params.content_changes;

        let mut workspace = self.workspace().write().await;

        workspace.edit_lsp(&uri, changes, self.const_config().position_encoding);

        drop(workspace);

        if let Err(err) = self.on_source_changed(&uri).await {
            error!(%err, %uri, "could not handle source change");
        };
    }

    #[tracing::instrument(skip_all, fields(uri = %params.text_document.uri))]
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        let config = self.config.read().await;

        if config.export_pdf == ExportPdfMode::OnSave {
            if let Err(err) = self.run_diagnostics_and_export(&uri).await {
                error!(%err, %uri, "could not handle source save");
            };
        }
    }

    #[tracing::instrument(skip(self))]
    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let changes = params.changes;

        let mut workspace = self.workspace().write().await;

        for change in changes {
            self.handle_file_change_event(&mut workspace, change);
        }
    }

    #[tracing::instrument(skip(self))]
    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        let event = params.event;

        let mut workspace = self.workspace().write().await;

        if let Err(err) = workspace.handle_workspace_folders_change_event(&event) {
            error!(%err, "error when changing workspace folders");
        }
    }

    #[tracing::instrument(
        skip_all,
        fields(command = params.command, arguments = ?params.arguments)
    )]
    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> jsonrpc::Result<Option<JsonValue>> {
        let ExecuteCommandParams {
            command,
            arguments,
            work_done_progress_params: _,
        } = params;
        match LspCommand::parse(&command) {
            Some(LspCommand::ExportPdf) => {
                self.command_export_pdf(arguments).await?;
            }
            Some(LspCommand::ClearCache) => {
                self.command_clear_cache(arguments).await?;
            }
            None => {
                error!("asked to execute unknown command");
                return Err(jsonrpc::Error::method_not_found());
            }
        };
        Ok(None)
    }

    #[tracing::instrument(
        skip_all,
        fields(
            uri = %params.text_document_position_params.text_document.uri,
            position = ?params.text_document_position_params.position,
        )
    )]
    async fn hover(&self, params: HoverParams) -> jsonrpc::Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        self.get_hover(&uri, position).await.map_err(|err| {
            error!(%err, %uri, "error getting hover");
            jsonrpc::Error::internal_error()
        })
    }

    #[tracing::instrument(
        skip_all,
        fields(
            uri = %params.text_document_position.text_document.uri,
            position = ?params.text_document_position.position,
        )
    )]
    async fn completion(
        &self,
        params: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        // FIXME: correctly identify a completion which is triggered
        // by explicit action, such as by pressing control and space
        // or something similar.
        //
        // See <https://github.com/microsoft/language-server-protocol/issues/1101>
        // > As of LSP 3.16, CompletionTriggerKind takes the value Invoked for
        // > both manually invoked (for ex: ctrl + space in VSCode) completions
        // > and always on (what the spec refers to as 24/7 completions).
        //
        // Hence, we cannot distinguish between the two cases. Conservatively, we
        // assume that the completion is not explicit.
        let explicit = false;

        let position_encoding = self.const_config().position_encoding;
        let doc = { self.document.lock().await.clone() };
        let completions = self
            .thread_with_world(&uri)
            .await
            .map_err(|err| {
                error!(%err, %uri, "error getting completion");
                jsonrpc::Error::internal_error()
            })?
            .run(move |world| {
                let source = world.main();

                let typst_offset =
                    lsp_to_typst::position_to_offset(position, position_encoding, &source);
                let (typst_start_offset, completions) =
                    typst_ide::autocomplete(&world, Some(&doc), &source, typst_offset, explicit)?;
                let lsp_start_position =
                    offset_to_position(typst_start_offset, position_encoding, &source);

                Some((lsp_start_position, completions))
            })
            .await
            .map(|(start_position, completions)| {
                let replace_range = LspRawRange::new(start_position, position);
                typst_to_lsp::completions(&completions, replace_range).into()
            });
        Ok(completions)
    }

    #[tracing::instrument(
        skip_all,
        fields(
            uri = %params.text_document_position_params.text_document.uri,
            position = ?params.text_document_position_params.position,
        )
    )]
    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> jsonrpc::Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        self.get_signature_at_position(&uri, position)
            .await
            .map_err(|err| {
                error!(%err, %uri, "error getting signature");
                jsonrpc::Error::internal_error()
            })
    }

    #[tracing::instrument(skip_all, fields(uri = %params.text_document.uri))]
    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> jsonrpc::Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;

        let symbols: Vec<_> = self
            .scope_with_source(&uri)
            .await
            .map_err(|err| {
                error!(%err, %uri, "error getting document symbols");
                jsonrpc::Error::internal_error()
            })?
            .run(|source, _| self.document_symbols(source, &uri, None).try_collect())
            .map_err(|err| {
                error!(%err, %uri, "failed to get document symbols");
                jsonrpc::Error::internal_error()
            })?;

        Ok(Some(symbols.into()))
    }

    #[tracing::instrument(skip_all, fields(query = params.query))]
    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> jsonrpc::Result<Option<Vec<SymbolInformation>>> {
        let handle_read_err = |err| warn!(%err, "could not read source");
        let handle_symbol_err = |err| {
            error!(%err, "failed to get document symbols");
            jsonrpc::Error::internal_error()
        };

        let query = (!params.query.is_empty()).then_some(params.query.as_str());

        let workspace = self.read_workspace().await;

        let uris = workspace.known_uris();

        trace!(?uris, "getting sources for these URIs");

        let uris_sources = uris
            .into_iter()
            .map(|uri| workspace.read_source(&uri).map(|source| (uri, source)))
            .map(|result| result.map_err(handle_read_err))
            .filter_map(Result::ok)
            .collect_vec();

        trace!(?uris_sources, "getting symbols for these sources");

        let symbols = uris_sources
            .iter()
            .flat_map(|(uri, source)| self.document_symbols(source, uri, query))
            .try_collect()
            .map_err(handle_symbol_err);

        trace!(?symbols, "got symbols");

        Some(symbols).transpose()
    }

    #[tracing::instrument(skip_all, fields(uri = %params.text_document.uri))]
    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> jsonrpc::Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;

        let (tokens, result_id) = self
            .scope_with_source(&uri)
            .await
            .map_err(|err| {
                error!(%err, %uri, "error getting full semantic tokens");
                jsonrpc::Error::internal_error()
            })?
            .run(|source, _| self.get_semantic_tokens_full(source));

        Ok(Some(
            SemanticTokens {
                result_id: Some(result_id),
                data: tokens,
            }
            .into(),
        ))
    }

    #[tracing::instrument(skip_all, fields(uri = %params.text_document.uri))]
    async fn semantic_tokens_full_delta(
        &self,
        params: SemanticTokensDeltaParams,
    ) -> jsonrpc::Result<Option<SemanticTokensFullDeltaResult>> {
        let uri = params.text_document.uri;
        let previous_result_id = params.previous_result_id;

        let scope = self.scope_with_source(&uri).await.map_err(|err| {
            error!(%err, %uri, "error getting semantic token delta");
            jsonrpc::Error::internal_error()
        })?;
        scope.run(|source, _| {
            let (tokens, result_id) =
                self.try_semantic_tokens_delta_from_result_id(source, &previous_result_id);
            match tokens {
                Ok(edits) => Ok(Some(
                    SemanticTokensDelta {
                        result_id: Some(result_id),
                        edits,
                    }
                    .into(),
                )),
                Err(tokens) => Ok(Some(
                    SemanticTokens {
                        result_id: Some(result_id),
                        data: tokens,
                    }
                    .into(),
                )),
            }
        })
    }

    #[tracing::instrument(skip(self))]
    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        // For some clients, we don't get the actual changed configuration and need to poll for it
        // https://github.com/microsoft/language-server-protocol/issues/676
        let values = match params.settings {
            JsonValue::Object(settings) => Ok(settings),
            _ => self
                .client
                .configuration(Config::get_items())
                .await
                .map(Config::values_to_map),
        };

        let result = match values {
            Ok(values) => {
                let mut config = self.config.write().await;
                config.update_by_map(&values).await
            }
            Err(err) => Err(err.into()),
        };

        match result {
            Ok(()) => {
                info!("new settings applied");
            }
            Err(err) => {
                error!(%err, "error applying new settings");
            }
        }
    }

    #[tracing::instrument(skip(self))]
    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> jsonrpc::Result<Option<Vec<SelectionRange>>> {
        let uri = params.text_document.uri;
        let positions = params.positions;

        let selection_range = self
            .scope_with_source(&uri)
            .await
            .map_err(|err| {
                error!(%err, %uri, "error getting selection range");
                jsonrpc::Error::internal_error()
            })?
            .run(|source, _| self.get_selection_range(source, &positions));

        Ok(selection_range)
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;
        let folding_ranges = self
            .scope_with_source(&uri)
            .await
            .map_err(|err| {
                error!(%err, %uri, "error getting folding ranges");
                jsonrpc::Error::internal_error()
            })?
            .run(|source, _| self.get_folding_ranges(source));
        
        Ok(folding_ranges)
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> jsonrpc::Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;

        let edits = self
            .scope_with_source(&uri)
            .await
            .map_err(|err| {
                error!(%err, %uri, "error getting document to format");
                jsonrpc::Error::internal_error()
            })?
            .run2(|source, project| self.format_document(project, source))
            .await
            .map_err(|err| {
                error!(%err, %uri, "error formatting document");
                jsonrpc::Error::internal_error()
            })?;

        Ok(Some(edits))
    }
}
