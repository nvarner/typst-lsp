use anyhow::anyhow;
use futures::future::TryFutureExt;
use tower_lsp::lsp_types::{Position, Range, Registration, TextEdit, Unregistration};
use typst::{
    foundations::Bytes,
    syntax::{FileId, Source, VirtualPath},
};
use typstfmt_lib::Config;

use crate::workspace::{fs::FsResult, project::Project};

use super::TypstServer;

const FORMATTING_REGISTRATION_ID: &str = "formatting";
const DOCUMENT_FORMATTING_METHOD_ID: &str = "textDocument/formatting";
const CONFIG_PATH: &str = "typstfmt.toml";

pub fn get_formatting_registration() -> Registration {
    Registration {
        id: FORMATTING_REGISTRATION_ID.to_owned(),
        method: DOCUMENT_FORMATTING_METHOD_ID.to_owned(),
        register_options: None,
    }
}

pub fn get_formatting_unregistration() -> Unregistration {
    Unregistration {
        id: FORMATTING_REGISTRATION_ID.to_owned(),
        method: DOCUMENT_FORMATTING_METHOD_ID.to_owned(),
    }
}

impl TypstServer {
    pub async fn format_document(
        &self,
        project: Project,
        source: Source,
    ) -> anyhow::Result<Vec<TextEdit>> {
        let config = get_config(&project).await?;
        let original_text = source.text();
        let res = typstfmt_lib::format(original_text, config);

        Ok(vec![TextEdit {
            new_text: res,
            range: Range::new(
                Position {
                    line: 0,
                    character: 0,
                },
                Position {
                    line: u32::MAX,
                    character: u32::MAX,
                },
            ),
        }])
    }
}

async fn get_config(project: &Project) -> anyhow::Result<Config> {
    config_from_file(project)
        .await
        .unwrap_or_else(|| Ok(Config::default()))
}

async fn config_from_file(project: &Project) -> Option<anyhow::Result<Config>> {
    async fn read_file(project: &Project, path: &str) -> FsResult<Bytes> {
        let file_id = FileId::new(None, VirtualPath::new(path));
        project.read_bytes_by_id(file_id).await
    }

    let file = read_file(project, CONFIG_PATH)
        .or_else(|_| async { read_file(project, &format!(".{CONFIG_PATH}")).await })
        .await
        .ok()?;
    let bytes = file.as_slice();
    Some(config_from_bytes(bytes))
}

fn config_from_bytes(bytes: &[u8]) -> anyhow::Result<Config> {
    let string = std::str::from_utf8(bytes)?;
    let config = Config::from_toml(string).map_err(|err| anyhow!("{err}"))?;
    Ok(config)
}
