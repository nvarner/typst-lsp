use std::sync::Arc;

use itertools::Itertools;
use once_cell::sync::OnceCell;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::{InitializeParams, Url};
use tower_lsp::{jsonrpc, Client};
use tracing::info;
use tracing_subscriber::{reload, Registry};
use typst::diag::FileResult;
use typst::syntax::SourceId;

use crate::config::{Config, ConstConfig};
use crate::lsp_typst_boundary::world::WorkspaceWorld;
use crate::server::semantic_tokens::SemanticTokenCache;
use crate::workspace::Workspace;

use self::log::LspLayer;

pub mod command;
pub mod diagnostics;
pub mod document;
pub mod export;
pub mod hover;
pub mod log;
pub mod lsp;
pub mod selection_range;
pub mod semantic_tokens;
pub mod signature;
pub mod symbols;
pub mod typst_compiler;
pub mod watch;

pub struct TypstServer {
    client: Client,
    workspace: Arc<RwLock<Workspace>>,
    config: Arc<RwLock<Config>>,
    const_config: OnceCell<ConstConfig>,
    semantic_tokens_delta_cache: Arc<parking_lot::RwLock<SemanticTokenCache>>,
    lsp_tracing_layer_handle: reload::Handle<Option<LspLayer>, Registry>,
}

impl TypstServer {
    pub fn new(
        client: Client,
        lsp_tracing_layer_handle: reload::Handle<Option<LspLayer>, Registry>,
    ) -> Self {
        Self {
            client,
            workspace: Default::default(),
            config: Default::default(),
            const_config: Default::default(),
            semantic_tokens_delta_cache: Default::default(),
            lsp_tracing_layer_handle,
        }
    }

    pub fn get_const_config(&self) -> &ConstConfig {
        self.const_config
            .get()
            .expect("const config should be initialized")
    }

    pub async fn get_world_with_main(&self, main_uri: Url) -> FileResult<WorkspaceWorld> {
        let workspace = self.workspace.read().await;
        let main_id = workspace.sources.get_id_by_uri(main_uri)?;
        drop(workspace);

        Ok(self.get_world_with_main_by_id(main_id).await)
    }

    async fn get_world_with_main_by_id(&self, main: SourceId) -> WorkspaceWorld {
        let config = self.config.read().await;
        WorkspaceWorld::new(
            Arc::clone(&self.workspace).read_owned().await,
            main,
            config.root_path.clone(),
        )
    }

    #[tracing::instrument(skip(self))]
    pub async fn register_workspace_files(&self, params: &InitializeParams) -> jsonrpc::Result<()> {
        let workspace = self.workspace.read().await;
        let source_manager = &workspace.sources;

        let workspace_uris = params
            .workspace_folders
            .iter()
            .flat_map(|folders| folders.iter())
            .map(|folder| &folder.uri);

        let root_uri = params.root_uri.iter();

        let uris_to_register = workspace_uris.chain(root_uri).unique_by(|x| *x);

        for uri in uris_to_register {
            source_manager.register_workspace_files(uri).map_err(|e| {
                jsonrpc::Error::invalid_params(format!("failed to register workspace files: {e:#}"))
            })?;
            info!(%uri, "folder added to workspace");
        }

        Ok(())
    }
}
