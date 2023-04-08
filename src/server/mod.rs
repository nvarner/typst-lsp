use std::sync::Arc;

use once_cell::sync::OnceCell;
use tokio::sync::RwLock;
use tower_lsp::Client;

use crate::config::{Config, ConstConfig};
use crate::workspace::Workspace;

pub mod command;
pub mod diagnostics;
pub mod document;
pub mod export;
pub mod log;
pub mod lsp;
pub mod signature;
pub mod typst_compiler;

pub struct TypstServer {
    client: Client,
    workspace: RwLock<Workspace>,
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
}
