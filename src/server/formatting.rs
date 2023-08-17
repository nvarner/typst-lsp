use tower_lsp::lsp_types::{Position, Range, Registration, TextEdit, Unregistration};
use typst::syntax::Source;

use super::TypstServer;

const FORMATTING_REGISTRATION_ID: &str = "formatting";
const DOCUMENT_FORMATTING_METHOD_ID: &str = "textDocument/formatting";

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
        let res = typstfmt_lib::format(original_text, typstfmt_lib::Config::default());

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
