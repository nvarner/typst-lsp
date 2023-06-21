use tower_lsp::lsp_types::{
    DidChangeWatchedFilesRegistrationOptions, FileEvent, FileSystemWatcher, GlobPattern,
    Registration,
};

use crate::lsp_typst_boundary::lsp_to_typst;
use crate::workspace::Workspace;

use super::TypstServer;

static WATCH_TYPST_FILES_REGISTRATION_ID: &str = "watch_typst_files";
static WATCH_FILES_METHOD: &str = "workspace/didChangeWatchedFiles";

impl TypstServer {
    pub fn get_watcher_registration(&self) -> Registration {
        Registration {
            id: WATCH_TYPST_FILES_REGISTRATION_ID.to_owned(),
            method: WATCH_FILES_METHOD.to_owned(),
            register_options: Some(
                serde_json::to_value(DidChangeWatchedFilesRegistrationOptions {
                    watchers: vec![FileSystemWatcher {
                        glob_pattern: GlobPattern::String("**/*".to_owned()),
                        kind: None,
                    }],
                })
                .unwrap(),
            ),
        }
    }

    pub fn handle_file_change_event(&self, workspace: &mut Workspace, event: FileEvent) {
        let uri = event.uri;

        let path = lsp_to_typst::uri_to_path(&uri);

        let is_typst = path
            .ok()
            .and_then(|path| path.extension().map(|extension| extension == "typ"))
            .unwrap_or(false);

        if is_typst {
            workspace.sources.invalidate(&uri);
        } else {
            workspace.resources.write().invalidate(uri);
        }
    }
}
