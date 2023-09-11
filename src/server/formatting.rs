use tokio::fs::File;
use tokio::io::AsyncReadExt;

use tower_lsp::lsp_types::{Position, Range, Registration, TextEdit, Unregistration};
use typst::syntax::Source;
use typstfmt_lib::Config as FmtConfig;

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
    pub async fn format_document(&self, source: &Source) -> anyhow::Result<Vec<TextEdit>> {
        let original_text = source.text();
        let res = typstfmt_lib::format(original_text, self.get_fmt_config().await);

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

    async fn get_fmt_config(&self) -> FmtConfig {
        // Ignoring all errors since we're returning the default config in case
        // we can't find something more specific
        let mut config_file: Option<File> = File::options().read(true).open(CONFIG_PATH).await.ok();

        if config_file.is_none() {
            if let Some(root_path) = &self.config.read().await.root_path {
                let mut root_path = root_path.clone();
                root_path.push(CONFIG_PATH);
                config_file = File::options().read(true).open(root_path).await.ok();
            }
        }

        if let Some(mut f) = config_file {
            let mut buf = String::default();
            let _ = f.read_to_string(&mut buf).await;
            FmtConfig::from_toml(&buf).unwrap_or_default()
        } else {
            FmtConfig::default()
        }
    }
}
