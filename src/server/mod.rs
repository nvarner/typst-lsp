use std::sync::Arc;

use once_cell::sync::OnceCell;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::Url;
use tower_lsp::Client;

use crate::config::{Config, ConstConfig};
use crate::lsp_typst_boundary::world::WorkspaceWorld;
use crate::workspace::source_manager::SourceId;
use crate::workspace::Workspace;

pub mod command;
pub mod diagnostics;
pub mod document;
pub mod export;
pub mod hover;
pub mod log;
pub mod lsp;
pub mod signature;
pub mod symbols;
pub mod typst_compiler;
pub mod watch;

pub struct TypstServer {
    client: Client,
    workspace: Arc<RwLock<Workspace>>,
    config: Arc<RwLock<Config>>,
    const_config: OnceCell<ConstConfig>,
}

impl TypstServer {
    pub fn with_client(client: Client) -> Self {
        Self {
            client,
            workspace: Default::default(),
            config: Default::default(),
            const_config: Default::default(),
        }
    }

    pub fn get_const_config(&self) -> &ConstConfig {
        self.const_config
            .get()
            .expect("const config should be initialized")
    }

    pub async fn get_world_with_main_uri(&self, main: &Url) -> (WorkspaceWorld, SourceId) {
        let workspace = self.workspace.read().await;
        let source_id = workspace
            .sources
            .get_id_by_uri(main)
            .expect("source should exist");
        drop(workspace);
        (self.get_world_with_main(source_id).await, source_id)
    }

    pub async fn get_world_with_main(&self, main: SourceId) -> WorkspaceWorld {
        WorkspaceWorld::new(Arc::clone(&self.workspace).read_owned().await, main)
    }
}
