use std::sync::Arc;

use anyhow::Context;
use futures::FutureExt;
use itertools::Itertools;
use serde_json::Value as JsonValue;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::*;
use tower_lsp::{jsonrpc, LanguageServer};
use tracing::{error, info, trace, warn};
use typst::ide::autocomplete;
use typst::World;

use crate::config::{
    get_config_registration, Config, ConstConfig, ExportPdfMode, SemanticTokensMode,
};
use crate::ext::InitializeParamsExt;
use crate::lsp_typst_boundary::{lsp_to_typst, typst_to_lsp};
use crate::workspace::Workspace;

use super::command::LspCommand;
use super::semantic_tokens::{
    get_semantic_tokens_options, get_semantic_tokens_registration,
    get_semantic_tokens_unregistration,
};
use super::TypstServer;

#[tower_lsp::async_trait]
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

        self.register_workspace_files().await;

        let config = self.config.read().await;
        let semantic_tokens_provider = match config.semantic_tokens {
            SemanticTokensMode::Enable
                if !params.supports_semantic_tokens_dynamic_registration() =>
            {
                Some(get_semantic_tokens_options().into())
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
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    ..Default::default()
                }),
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

        let config = self.config.read().await;

        let world = match self.world_with_main(&uri).await {
            Ok(world) => world,
            Err(err) => {
                error!(%err, %uri, "could not get world");
                return;
            }
        };
        let source = world.main();

        self.on_source_changed(&world, &config, &source).await;
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

        let config = self.config.read().await;
        let world = self.world_with_main(&uri).await;
        let world = match world {
            Ok(world) => world,
            Err(err) => {
                error!(%err, %uri, "could not get world");
                return;
            }
        };
        let source = world.main();

        self.on_source_changed(&world, &config, &source).await;
    }

    #[tracing::instrument(skip_all, fields(uri = %params.text_document.uri))]
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        let config = self.config.read().await;

        if config.export_pdf == ExportPdfMode::OnSave {
            let world = self.world_with_main(&uri).await;
            let world = match world {
                Ok(world) => world,
                Err(err) => {
                    error!(%err, %uri, "could not get world");
                    return;
                }
            };
            let source = world.main();

            self.run_diagnostics_and_export(&world, &source).await;
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

        workspace.handle_workspace_folders_change_event(&event);
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

        let world = self.world_with_main(&uri).await.map_err(|err| {
            error!(%err, %uri, "could not get world");
            jsonrpc::Error::internal_error()
        })?;
        let source = world.main();

        Ok(self.get_hover(&world, &source, position))
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
        let explicit = params
            .context
            .map(|context| context.trigger_kind == CompletionTriggerKind::INVOKED)
            .unwrap_or(false);

        let world = self.world_with_main(&uri).await.map_err(|err| {
            error!(%err, %uri, "could not get world");
            jsonrpc::Error::internal_error()
        })?;
        let source = world.main();

        let typst_offset = lsp_to_typst::position_to_offset(
            position,
            self.const_config().position_encoding,
            &source,
        );

        let completions = autocomplete(&world, &[], &source, typst_offset, explicit)
            .map(|(_, completions)| typst_to_lsp::completions(&completions).into());
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

        let world = self.world_with_main(&uri).await.map_err(|err| {
            error!(%err, %uri, "could not get world");
            jsonrpc::Error::internal_error()
        })?;
        let source = world.main();

        Ok(self.get_signature_at_position(&world, &source, position))
    }

    #[tracing::instrument(skip_all, fields(uri = %params.text_document.uri))]
    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> jsonrpc::Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;

        let workspace = self.workspace().read().await;

        let source = workspace.read_source(&uri).map_err(|err| {
            error!(%err, %uri, "could not open file while getting document symbols");
            jsonrpc::Error::internal_error()
        })?;

        let symbols: Vec<_> = self
            .document_symbols(&source, &uri, None)
            .try_collect()
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

        let workspace = self.workspace().read().await;

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

        let workspace = self.workspace().read().await;

        let source = workspace.read_source(&uri).map_err(|err| {
            error!(%err, %uri, "could not open file while getting full semantic tokens");
            jsonrpc::Error::internal_error()
        })?;

        let (tokens, result_id) = self.get_semantic_tokens_full(&source);

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

        let workspace = self.workspace().read().await;

        let source = workspace.read_source(&uri).map_err(|err| {
            error!(%err, %uri, "could not open file while getting full semantic tokens delta");
            jsonrpc::Error::internal_error()
        })?;

        let (tokens, result_id) =
            self.try_semantic_tokens_delta_from_result_id(&source, &previous_result_id);
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
    }

    #[tracing::instrument(skip(self))]
    async fn did_change_configuration(&self, _params: DidChangeConfigurationParams) {
        // We don't get the actual changed configuration and need to poll for it
        // https://github.com/microsoft/language-server-protocol/issues/676

        let values = self
            .client
            .configuration(Config::get_items())
            .await
            .unwrap();

        let mut config = self.config.write().await;
        match config.update_from_values(values).await {
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

        let world = self.world_with_main(&uri).await.map_err(|err| {
            error!(%err, %uri, "could not get world");
            jsonrpc::Error::internal_error()
        })?;
        let source = world.main();

        Ok(self.get_selection_range(&source, &positions))
    }
}
