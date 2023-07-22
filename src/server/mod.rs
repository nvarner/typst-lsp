use std::sync::Arc;

use itertools::Itertools;
use once_cell::sync::OnceCell;
use tokio::sync::{Mutex, RwLock};
use tower_lsp::lsp_types::{InitializeParams, Url};
use tower_lsp::{jsonrpc, Client};
use tracing::{error, info};
use tracing_subscriber::{reload, Registry};
use typst::diag::{FileError, FileResult};
use typst::file::FileId;

use crate::config::{Config, ConstConfig};
use crate::lsp_typst_boundary::world::ProjectWorld;
use crate::server::semantic_tokens::SemanticTokenCache;
use crate::workspace::project::Project;
use crate::workspace::Workspace;

use self::diagnostics::DiagnosticsManager;
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
    workspace: OnceCell<Arc<RwLock<Workspace>>>,
    config: Arc<RwLock<Config>>,
    const_config: OnceCell<ConstConfig>,
    semantic_tokens_delta_cache: Arc<parking_lot::RwLock<SemanticTokenCache>>,
    diagnostics: Mutex<DiagnosticsManager>,
    lsp_tracing_layer_handle: reload::Handle<Option<LspLayer>, Registry>,
}

impl TypstServer {
    pub fn new(
        client: Client,
        lsp_tracing_layer_handle: reload::Handle<Option<LspLayer>, Registry>,
    ) -> Self {
        Self {
            workspace: Default::default(),
            config: Default::default(),
            const_config: Default::default(),
            semantic_tokens_delta_cache: Default::default(),
            diagnostics: Mutex::new(DiagnosticsManager::new(client.clone())),
            lsp_tracing_layer_handle,
            client,
        }
    }

    pub fn const_config(&self) -> &ConstConfig {
        self.const_config
            .get()
            .expect("const config should be initialized")
    }

    pub fn workspace(&self) -> &Arc<RwLock<Workspace>> {
        self.workspace
            .get()
            .expect("workspace should be initialized")
    }

    pub async fn get_world_with_main(&self, main_uri: Url) -> FileResult<ProjectWorld> {
        let workspace = self.workspace().read().await;
        let main_id = workspace.uri_to_id(&main_uri).map_err(|err| {
            error!(%err, %main_uri, "couldn't get id for main URI");
            FileError::Other
        })?;
        drop(workspace);

        Ok(self.get_world_with_main_by_id(main_id).await)
    }

    async fn get_world_with_main_by_id(&self, main: FileId) -> ProjectWorld {
        ProjectWorld::new(Arc::clone(self.workspace()).read_owned().await, main)
    }

    #[tracing::instrument(skip(self))]
    pub async fn register_workspace_files(&self, params: &InitializeParams) -> jsonrpc::Result<()> {
        let mut workspace = self.workspace().write().await;

        let workspace_uris = params
            .workspace_folders
            .iter()
            .flat_map(|folders| folders.iter())
            .map(|folder| &folder.uri);

        let root_uri = params.root_uri.iter();

        let uris_to_register = workspace_uris.chain(root_uri).unique_by(|x| *x);

        // TODO: replace this

        // for uri in uris_to_register {
        //     workspace
        //         .source_manager_mut()
        //         .register_workspace_files(uri)
        //         .map_err(|e| {
        //             jsonrpc::Error::invalid_params(format!(
        //                 "failed to register workspace files: {e:#}"
        //             ))
        //         })?;
        //     info!(%uri, "folder added to workspace");
        // }

        Ok(())
    }
}
