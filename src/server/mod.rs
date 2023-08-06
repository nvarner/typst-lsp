use std::sync::Arc;

use once_cell::sync::OnceCell;
use tokio::runtime;
use tokio::sync::{Mutex, OwnedRwLockReadGuard, RwLock, RwLockReadGuard};
use tower_lsp::lsp_types::Url;
use tower_lsp::Client;
use tracing_subscriber::{reload, Registry};
use typst::syntax::Source;

use crate::config::{Config, ConstConfig};
use crate::server::semantic_tokens::SemanticTokenCache;
use crate::workspace::fs::FsResult;
use crate::workspace::package::FullFileId;
use crate::workspace::project::Project;
use crate::workspace::world::typst_thread::TypstThread;
use crate::workspace::world::ProjectWorld;
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
    typst_thread: TypstThread,
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
            typst_thread: Default::default(),
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

    pub async fn typst_global_scope(&self) -> typst::eval::Scope {
        self.read_workspace()
            .await
            .typst_stdlib
            .global
            .scope()
            .clone()
    }

    #[tracing::instrument(skip(self))]
    pub async fn register_workspace_files(&self) -> FsResult<()> {
        let mut workspace = self.workspace().write().await;
        workspace.register_files()
    }

    async fn read_workspace(&self) -> RwLockReadGuard<Workspace> {
        self.workspace().read().await
    }

    async fn read_workspace_owned(&self) -> OwnedRwLockReadGuard<Workspace> {
        Arc::clone(self.workspace()).read_owned().await
    }

    pub async fn project_and_full_id(&self, uri: &Url) -> FsResult<(Project, FullFileId)> {
        let workspace = self.read_workspace_owned().await;
        let full_id = workspace.full_id(uri)?;
        let project = Project::new(full_id.package(), workspace);
        Ok((project, full_id))
    }

    pub async fn scope_with_source(&self, uri: &Url) -> FsResult<SourceScope> {
        let (project, _) = self.project_and_full_id(uri).await?;
        let source = project.read_source_by_uri(uri)?;
        Ok(SourceScope { project, source })
    }

    pub async fn thread_with_world(
        &self,
        builder: impl Into<WorldBuilder<'_>>,
    ) -> FsResult<WorldThread> {
        let (main_project, main_uri) = builder.into().project_uri(self.workspace()).await?;

        Ok(WorldThread {
            main_project,
            main_uri,
            typst_thread: &self.typst_thread,
        })
    }

    pub async fn thread<T: Send + 'static>(
        &self,
        f: impl FnOnce(runtime::Handle) -> T + Send + 'static,
    ) -> T {
        self.typst_thread.run(f).await
    }
}

pub struct SourceScope {
    project: Project,
    source: Source,
}

impl SourceScope {
    pub fn run<T>(self, f: impl FnOnce(&Source, &Project) -> T) -> T {
        f(&self.source, &self.project)
    }
}

pub struct WorldThread<'a> {
    main_project: Project,
    main_uri: Url,
    typst_thread: &'a TypstThread,
}

impl<'a> WorldThread<'a> {
    pub async fn run<T: Send + 'static>(
        self,
        f: impl FnOnce(ProjectWorld) -> T + Send + 'static,
    ) -> T {
        self.typst_thread
            .run_with_world(self.main_project, self.main_uri, f)
            .await
    }
}

pub enum WorldBuilder<'a> {
    MainFullId(FullFileId),
    MainUri(&'a Url),
    MainFullIdAndUri(FullFileId, &'a Url),
    ProjectAndMainUri(Project, &'a Url),
}

impl<'a> WorldBuilder<'a> {
    async fn project_uri(self, workspace: &Arc<RwLock<Workspace>>) -> FsResult<(Project, Url)> {
        match self {
            Self::MainFullId(full_id) => {
                let workspace = Arc::clone(workspace).read_owned().await;
                let uri = workspace.uri(full_id).await?;
                Ok((Project::new(full_id.package(), workspace), uri))
            }
            Self::MainUri(uri) => {
                let workspace = Arc::clone(workspace).read_owned().await;
                let full_id = workspace.full_id(uri)?;
                Ok((Project::new(full_id.package(), workspace), uri.clone()))
            }
            Self::MainFullIdAndUri(full_id, uri) => {
                let workspace = Arc::clone(workspace).read_owned().await;
                Ok((Project::new(full_id.package(), workspace), uri.clone()))
            }
            Self::ProjectAndMainUri(project, uri) => Ok((project, uri.clone())),
        }
    }
}

impl<'a> From<FullFileId> for WorldBuilder<'a> {
    fn from(full_id: FullFileId) -> Self {
        Self::MainFullId(full_id)
    }
}

impl<'a> From<&'a Url> for WorldBuilder<'a> {
    fn from(uri: &'a Url) -> Self {
        Self::MainUri(uri)
    }
}

impl<'a> From<(FullFileId, &'a Url)> for WorldBuilder<'a> {
    fn from((full_id, uri): (FullFileId, &'a Url)) -> Self {
        Self::MainFullIdAndUri(full_id, uri)
    }
}

impl<'a> From<(Project, &'a Url)> for WorldBuilder<'a> {
    fn from((project, uri): (Project, &'a Url)) -> Self {
        Self::ProjectAndMainUri(project, uri)
    }
}
