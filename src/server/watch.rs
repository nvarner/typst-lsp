use serde_json::to_value;
use tower_lsp::lsp_types::{
    DidChangeWatchedFilesRegistrationOptions, FileEvent, FileSystemWatcher, GlobPattern,
    Registration,
};

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
                to_value(DidChangeWatchedFilesRegistrationOptions {
                    watchers: vec![FileSystemWatcher {
                        glob_pattern: GlobPattern::String("**/*.typ".to_owned()),
                        kind: None,
                    }],
                })
                .unwrap(),
            ),
        }
    }

    pub fn handle_file_change_event(&self, workspace: &mut Workspace, event: FileEvent) {
        workspace.sources.invalidate_closed(event.uri);
    }
}
