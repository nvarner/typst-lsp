use anyhow::anyhow;
use serde::Deserialize;
use serde_json::{Map, Value};
use tower_lsp::lsp_types::{self, InitializeParams, PositionEncodingKind};

use crate::ext::InitializeParamsExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportPdfMode {
    Never,
    #[default]
    OnSave,
    OnType,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub export_pdf: ExportPdfMode,
}

impl Config {
    pub fn update(&mut self, update: &Value) -> anyhow::Result<()> {
        if let Value::Object(update) = update {
            self.update_by_map(update);
            Ok(())
        } else {
            Err(anyhow!("got invalid configuration object {update}"))
        }
    }

    fn update_by_map(&mut self, update: &Map<String, Value>) {
        let export_pdf = update
            .get("exportPdf")
            .map(ExportPdfMode::deserialize)
            .and_then(Result::ok);
        if let Some(export_pdf) = export_pdf {
            self.export_pdf = export_pdf;
        }
    }
}

/// What counts as "1 character" for string indexing. We should always prefer UTF-8, but support
/// UTF-16 as long as it is standard. For more background on encodings and LSP, try
/// ["The bottom emoji breaks rust-analyzer"](https://fasterthanli.me/articles/the-bottom-emoji-breaks-rust-analyzer),
/// a well-written article on the topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionEncoding {
    /// "1 character" means "1 UTF-16 code unit"
    ///
    /// This is the only required encoding for LSPs to support, but it's not a natural one (unless
    /// you're working in JS). Prefer UTF-8, and refer to the article linked in the
    /// `PositionEncoding` docs for more background.
    #[default]
    Utf16,
    /// "1 character" means "1 byte"
    Utf8,
}

impl From<PositionEncoding> for lsp_types::PositionEncodingKind {
    fn from(position_encoding: PositionEncoding) -> Self {
        match position_encoding {
            PositionEncoding::Utf16 => Self::UTF16,
            PositionEncoding::Utf8 => Self::UTF8,
        }
    }
}

/// Configuration set at initialization that won't change within a single session
#[derive(Debug)]
pub struct ConstConfig {
    pub position_encoding: PositionEncoding,
    pub supports_multiline_tokens: bool,
}

impl ConstConfig {
    fn choose_encoding(params: &InitializeParams) -> PositionEncoding {
        let encodings = params.position_encodings();
        if encodings.contains(&PositionEncodingKind::UTF8) {
            PositionEncoding::Utf8
        } else {
            PositionEncoding::Utf16
        }
    }
}

impl From<&InitializeParams> for ConstConfig {
    fn from(params: &InitializeParams) -> Self {
        let position_encoding = Self::choose_encoding(params);
        let supports_multiline_tokens = params.supports_multiline_tokens();

        Self {
            position_encoding,
            supports_multiline_tokens,
        }
    }
}
