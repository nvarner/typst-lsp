use std::sync::Arc;

use once_cell::sync::OnceCell;
use tokio::runtime;
use tokio::sync::{Mutex, OwnedRwLockReadGuard, RwLock, RwLockReadGuard};
use tower_lsp::lsp_types::Url;
use tower_lsp::Client;
use tracing_subscriber::{reload, Registry};
use typst::model::Document;
use typst::syntax::Source;

use crate::config::{Config, ConstConfig};
use crate::server::semantic_tokens::SemanticTokenCache;
use crate::workspace::fs::FsResult;
use crate::workspace::package::FullFileId;
use crate::workspace::project::Project;
use crate::workspace::world::typst_thread::TypstThread;
use crate::workspace::world::ProjectWorld;
use crate::workspace::{Workspace, TYPST_STDLIB};

use self::diagnostics::DiagnosticsManager;
use self::log::LspLayer;

pub mod command;
pub mod diagnostics;
pub mod document;
pub mod export;
pub mod formatting;
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
    document: Mutex<Arc<Document>>,
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
            document: Default::default(),
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

    pub async fn main_url(&self) -> Option<Url> {
        self.config.read().await.main_file.clone()
    }

    pub fn typst_global_scopes(&self) -> typst::foundations::Scopes {
        typst::foundations::Scopes::new(Some(&TYPST_STDLIB))
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
        let (main, project) = builder.into().main_project(self.workspace()).await?;

        Ok(WorldThread {
            main,
            main_project: project,
            typst_thread: &self.typst_thread,
        })
    }

    /// Run the given function on the Typst thread, passing back its return value.
    pub async fn typst<T: Send + 'static>(
        &self,
        f: impl FnOnce(runtime::Handle) -> T + Send + 'static,
    ) -> T {
        self.typst_thread.run(f).await
    }
}

pub struct SourceScope {
    source: Source,
    project: Project,
}

impl SourceScope {
    pub fn run<T>(self, f: impl FnOnce(&Source, &Project) -> T) -> T {
        f(&self.source, &self.project)
    }

    pub fn run2<T>(self, f: impl FnOnce(Source, Project) -> T) -> T {
        f(self.source, self.project)
    }
}

pub struct WorldThread<'a> {
    main: Source,
    main_project: Project,
    typst_thread: &'a TypstThread,
}

impl<'a> WorldThread<'a> {
    pub async fn run<T: Send + 'static>(
        self,
        f: impl FnOnce(ProjectWorld) -> T + Send + 'static,
    ) -> T {
        self.typst_thread
            .run_with_world(self.main_project, self.main, f)
            .await
    }
}

pub enum WorldBuilder<'a> {
    MainUri(&'a Url),
    MainAndProject(Source, Project),
}

impl<'a> WorldBuilder<'a> {
    async fn main_project(self, workspace: &Arc<RwLock<Workspace>>) -> FsResult<(Source, Project)> {
        match self {
            Self::MainUri(uri) => {
                let workspace = Arc::clone(workspace).read_owned().await;
                let full_id = workspace.full_id(uri)?;
                let source = workspace.read_source(uri)?;
                let project = Project::new(full_id.package(), workspace);
                Ok((source, project))
            }
            Self::MainAndProject(main, project) => Ok((main, project)),
        }
    }
}

impl<'a> From<&'a Url> for WorldBuilder<'a> {
    fn from(uri: &'a Url) -> Self {
        Self::MainUri(uri)
    }
}

impl From<(Source, Project)> for WorldBuilder<'static> {
    fn from((main, project): (Source, Project)) -> Self {
        Self::MainAndProject(main, project)
    }
}
