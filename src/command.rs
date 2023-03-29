use std::{fs, path::Path};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::Value;
use tower_lsp::{
    jsonrpc::{Error, Result},
    lsp_types::{MessageType, Url},
};
use typst::geom::Color;

use crate::Backend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspCommand {
    ExportPdf,
    GeneratePreview,
}

impl From<LspCommand> for String {
    fn from(command: LspCommand) -> Self {
        match command {
            LspCommand::ExportPdf => "typst-lsp.doPdfExport".to_string(),
            LspCommand::GeneratePreview => "typst-lsp.generatePreview".to_string(),
        }
    }
}

impl LspCommand {
    pub fn parse(command: &str) -> Option<Self> {
        match command {
            "typst-lsp.doPdfExport" => Some(Self::ExportPdf),
            "typst-lsp.generatePreview" => Some(Self::GeneratePreview),
            _ => None,
        }
    }

    pub fn all_as_string() -> Vec<String> {
        vec![Self::ExportPdf.into()]
    }
}

fn validate_uri_argument(arguments: Vec<Value>) -> Result<Url> {
    if arguments.is_empty() {
        return Err(Error::invalid_params("Missing file URI argument"));
    }
    let Some(file_uri) = arguments.first().and_then(|v| v.as_str()) else {
        return Err(Error::invalid_params(
            "Missing file URI as first argument",
        ));
    };
    Url::parse(file_uri).map_err(|_| Error::invalid_params("Parameter is not a valid URI"))
}

/// Here are implemented the handlers for each command.
impl Backend {
    /// Export the current document as a PDF file. The client is reponsible for passing the correct file URI.
    pub async fn command_export_pdf(&self, arguments: Vec<Value>) -> Result<()> {
        let file_uri = validate_uri_argument(arguments)?;
        let text = fs::read_to_string(
            file_uri
                .to_file_path()
                .map_err(|_| Error::invalid_params("Could not convert file URI to path"))?,
        )
        .map_err(|_| Error::internal_error())?;
        self.compile_diags_export(file_uri, text, true).await;
        Ok(())
    }

    pub async fn command_generate_preview(&self, arguments: Vec<Value>) -> Result<Option<Value>> {
        let file_uri = validate_uri_argument(arguments)?;
        let text = fs::read_to_string(
            file_uri
                .to_file_path()
                .map_err(|_| Error::invalid_params("Could not convert file URI to path"))?,
        )
        .map_err(|_| Error::internal_error())?;

        let mut world_lock = self.world.write().await;
        let world = world_lock.as_mut().unwrap();

        world.reset();

        match world.resolve_with(Path::new(&file_uri.to_file_path().unwrap()), &text) {
            Ok(id) => {
                world.main = id;
            }
            Err(e) => {
                self.client
                    .log_message(MessageType::ERROR, format!("{:?}", e))
                    .await;
            }
        }

        match typst::compile(world) {
            Ok(document) => {
                let data_urls: Vec<serde_json::Value> = document
                    .pages
                    .iter()
                    .map(|frame| typst::export::render(frame, 1.5, Color::WHITE))
                    .map(|pixmap| pixmap.encode_png().unwrap())
                    .map(|buf| BASE64.encode(buf))
                    .map(|encoded| Value::String("data:image/png;base64,".to_string() + &encoded))
                    .collect();
 
                return Ok(Some(serde_json::Value::Array(data_urls)));
            }
            Err(_errors) => {}
        };

        Ok(None)
    }
}
