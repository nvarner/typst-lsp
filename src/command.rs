use tower_lsp::lsp_types::Url;

use crate::Backend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspCommand {
    ExportPdf,
}

impl From<LspCommand> for String {
    fn from(command: LspCommand) -> Self {
        match command {
            LspCommand::ExportPdf => "typst.exportPdf".to_string(),
        }
    }
}

impl LspCommand {
    pub fn parse(command: &str) -> Option<Self> {
        match command {
            "typst.exportPdf" => Some(Self::ExportPdf),
            _ => None,
        }
    }

    pub fn all_as_string() -> Vec<String> {
        vec![Self::ExportPdf.into()]
    }
}

/// Here are implemented the handlers for each command.
impl Backend {
    /// Export the current document as a PDF file. The client is reponsible for passing the correct file URI.
    pub async fn command_export_pdf(&self, file: Url, text: String) {
        self.compile_diags_export(file, text, true).await;
    }
}
