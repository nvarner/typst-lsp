use serde_json::Value;
use tower_lsp::jsonrpc;
use tower_lsp::{
    jsonrpc::{Error, Result},
    lsp_types::Url,
};
use tracing::error;

use super::TypstServer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspCommand {
    ExportPdf,
    ClearCache,
}

impl From<LspCommand> for String {
    fn from(command: LspCommand) -> Self {
        match command {
            LspCommand::ExportPdf => "typst-lsp.doPdfExport".to_string(),
            LspCommand::ClearCache => "typst-lsp.doClearCache".to_string(),
        }
    }
}

impl LspCommand {
    pub fn parse(command: &str) -> Option<Self> {
        match command {
            "typst-lsp.doPdfExport" => Some(Self::ExportPdf),
            "typst-lsp.doClearCache" => Some(Self::ClearCache),
            _ => None,
        }
    }

    pub fn all_as_string() -> Vec<String> {
        vec![Self::ExportPdf.into(), Self::ClearCache.into()]
    }
}

/// Here are implemented the handlers for each command.
impl TypstServer {
    /// Export the current document as a PDF file. The client is responsible for passing the correct file URI.
    #[tracing::instrument(skip(self))]
    pub async fn command_export_pdf(&self, arguments: Vec<Value>) -> Result<()> {
        if arguments.is_empty() {
            return Err(Error::invalid_params("Missing file URI argument"));
        }
        let Some(file_uri) = arguments.first().and_then(|v| v.as_str()) else {
            return Err(Error::invalid_params("Missing file URI as first argument"));
        };
        let file_uri = Url::parse(file_uri)
            .map_err(|_| Error::invalid_params("Parameter is not a valid URI"))?;

        self.run_export(&file_uri).await.map_err(|err| {
            error!(%err, "could not export PDF");
            jsonrpc::Error::internal_error()
        })?;

        Ok(())
    }

    /// Clear all cached resources.
    #[tracing::instrument(skip_all)]
    pub async fn command_clear_cache(&self, _arguments: Vec<Value>) -> Result<()> {
        self.workspace().write().await.clear().map_err(|err| {
            error!(%err, "could not clear cache");
            jsonrpc::Error::internal_error()
        })?;

        self.typst(|_| comemo::evict(0)).await;

        Ok(())
    }
}
