use std::sync::Arc;

use once_cell::sync::OnceCell;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::{InitializeParams, MessageType, Url};
use tower_lsp::{jsonrpc, Client};

use crate::config::{Config, ConstConfig};
use crate::lsp_typst_boundary::world::WorkspaceWorld;
use crate::server::log::LogMessage;
use crate::workspace::source_manager::SourceId;
use crate::workspace::Workspace;

pub mod command;
pub mod diagnostics;
pub mod document;
pub mod export;
pub mod hover;
pub mod log;
pub mod lsp;
pub mod selection_range;
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

    pub async fn register_workspace_files(&self, params: &InitializeParams) -> jsonrpc::Result<()> {
        let workspace = self.workspace.read().await;
        let source_manager = &workspace.sources;
        if let Some(workspace_folders) = &params.workspace_folders {
            for workspace_folder in workspace_folders {
                source_manager
                    .register_workspace_files(&workspace_folder.uri)
                    .map_err(|e| {
                        jsonrpc::Error::invalid_params(format!(
                            "failed to register workspace files: {e:#}"
                        ))
                    })?;
                self.log_to_client(LogMessage {
                    message_type: MessageType::INFO,
                    message: format!("Folder added to workspace: {}", &workspace_folder.uri),
                })
                .await;
            }
        }
        if let Some(root_uri) = &params.root_uri {
            source_manager
                .register_workspace_files(root_uri)
                .map_err(|e| {
                    jsonrpc::Error::invalid_params(format!(
                        "failed to register workspace files: {e:#}"
                    ))
                })?;
            self.log_to_client(LogMessage {
                message_type: MessageType::INFO,
                message: format!("Folder added to workspace: {}", &root_uri),
            })
            .await;
        }
        Ok(())
    }
}
