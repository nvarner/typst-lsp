use anyhow::anyhow;
use tower_lsp::lsp_types::{Position, Range, Registration, TextEdit, Unregistration};
use typst::syntax::{FileId, Source, VirtualPath};
use typstfmt_lib::Config;

use crate::workspace::project::Project;

use super::TypstServer;

const FORMATTING_REGISTRATION_ID: &str = "formatting";
const DOCUMENT_FORMATTING_METHOD_ID: &str = "textDocument/formatting";
const CONFIG_PATH: &str = "typstfmt-config.toml";

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
    let file_id = FileId::new(None, VirtualPath::new(CONFIG_PATH));
    let file = project.read_bytes_by_id(file_id).await.ok()?;
    let bytes = file.as_slice();
    Some(config_from_bytes(bytes))
}

fn config_from_bytes(bytes: &[u8]) -> anyhow::Result<Config> {
    let string = std::str::from_utf8(bytes)?;
    let config = Config::from_toml(string).map_err(|err| anyhow!("{err}"))?;
    Ok(config)
}
