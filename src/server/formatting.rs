use std::{fs::File, io::Read};

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
    pub fn format_document(&self, source: &Source) -> anyhow::Result<Vec<TextEdit>> {
        let original_text = source.text();
        let res = typstfmt_lib::format(original_text, self.get_fmt_config());

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

    fn get_fmt_config(&self) -> FmtConfig {
        // Ignoring all errors since we're returning the default config in case
        // we can't find something more specific
        let mut config_file: Option<File> = File::options().read(true).open(CONFIG_PATH).ok();

        if config_file.is_none() {
            if let Some(root_path) = &self.config.blocking_read().root_path {
                let mut root_path = root_path.clone();
                root_path.push(CONFIG_PATH);
                config_file = File::options().read(true).open(root_path).ok();
            }
        }

        config_file
            .map(|mut f| {
                let mut buf = String::default();
                let _ = f.read_to_string(&mut buf);
                FmtConfig::from_toml(&buf).ok()
            })
            .flatten()
            .unwrap_or_default()
    }
}
