use std::sync::Arc;

use once_cell::sync::OnceCell;
use tokio::sync::{Mutex, RwLock};
use tower_lsp::lsp_types::Url;
use tower_lsp::Client;
use tracing_subscriber::{reload, Registry};

use crate::config::{Config, ConstConfig};
use crate::lsp_typst_boundary::world::ProjectWorld;
use crate::server::semantic_tokens::SemanticTokenCache;
use crate::workspace::fs::FsResult;
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

    pub async fn world_with_main(&self, uri: &Url) -> FsResult<ProjectWorld> {
        let workspace = Arc::clone(self.workspace()).read_owned().await;
        let (meta, id) = workspace.uri_to_project_and_id(uri)?;
        let project = Project::new(workspace, meta);
        let world = ProjectWorld::new(project, id);
        Ok(world)
    }

    #[tracing::instrument(skip(self))]
    pub async fn register_workspace_files(&self) {
        let mut workspace = self.workspace().write().await;

        workspace.register_files();
    }
}
