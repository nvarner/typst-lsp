use std::{fmt, path::PathBuf};

use anyhow::bail;
use futures::future::BoxFuture;
use serde::Deserialize;
use serde_json::{Map, Value};
use tower_lsp::lsp_types::{
    self, ConfigurationItem, InitializeParams, PositionEncodingKind, Registration,
};

use crate::ext::InitializeParamsExt;

const CONFIG_REGISTRATION_ID: &str = "config";
const CONFIG_METHOD_ID: &str = "workspace/didChangeConfiguration";

pub fn get_config_registration() -> Registration {
    Registration {
        id: CONFIG_REGISTRATION_ID.to_owned(),
        method: CONFIG_METHOD_ID.to_owned(),
        register_options: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportPdfMode {
    Never,
    #[default]
    OnSave,
    OnType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SemanticTokensMode {
    Disable,
    #[default]
    Enable,
}

pub type Listener<T> = Box<dyn FnMut(&T) -> BoxFuture<anyhow::Result<()>> + Send + Sync>;

const CONFIG_ITEMS: &[&str] = &["exportPdf", "rootPath", "semanticTokens"];

#[derive(Default)]
pub struct Config {
    pub export_pdf: ExportPdfMode,
    pub root_path: Option<PathBuf>,
    pub semantic_tokens: SemanticTokensMode,
    semantic_tokens_listeners: Vec<Listener<SemanticTokensMode>>,
}

impl Config {
    pub fn get_items() -> Vec<ConfigurationItem> {
        CONFIG_ITEMS
            .iter()
            .map(|item| ConfigurationItem {
                section: Some(format!("typst-lsp.{item}")),
                ..Default::default()
            })
            .collect()
    }

    pub fn listen_semantic_tokens(&mut self, listener: Listener<SemanticTokensMode>) {
        self.semantic_tokens_listeners.push(listener);
    }

    pub async fn update(&mut self, update: &Value) -> anyhow::Result<()> {
        if let Value::Object(update) = update {
            self.update_by_map(update).await
        } else {
            bail!("got invalid configuration object {update}")
        }
    }

    pub async fn update_from_values(&mut self, update: Vec<Value>) -> anyhow::Result<()> {
        let items = CONFIG_ITEMS.iter().map(|item| (*item).to_owned());
        let map = items.zip(update.into_iter()).collect();
        self.update_by_map(&map).await
    }

    async fn update_by_map(&mut self, update: &Map<String, Value>) -> anyhow::Result<()> {
        let export_pdf = update
            .get("exportPdf")
            .map(ExportPdfMode::deserialize)
            .and_then(Result::ok);
        if let Some(export_pdf) = export_pdf {
            self.export_pdf = export_pdf;
        }

        let root_path = update.get("rootPath");
        if let Some(root_path) = root_path {
            if root_path.is_null() {
                self.root_path = None;
            }
            if let Some(root_path) = root_path.as_str().map(PathBuf::from) {
                self.root_path = Some(root_path);
            }
        }

        let semantic_tokens = update
            .get("semanticTokens")
            .map(SemanticTokensMode::deserialize)
            .and_then(Result::ok);
        if let Some(semantic_tokens) = semantic_tokens {
            for listener in &mut self.semantic_tokens_listeners {
                listener(&semantic_tokens).await?;
            }
            self.semantic_tokens = semantic_tokens;
        }

        Ok(())
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Config")
            .field("export_pdf", &self.export_pdf)
            .field("semantic_tokens", &self.semantic_tokens)
            .field(
                "semantic_tokens_listeners",
                &format_args!("Vec[len = {}]", self.semantic_tokens_listeners.len()),
            )
            .finish()
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
    pub supports_semantic_tokens_dynamic_registration: bool,
    pub supports_config_change_registration: bool,
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
        Self {
            position_encoding: Self::choose_encoding(params),
            supports_semantic_tokens_dynamic_registration: params
                .supports_semantic_tokens_dynamic_registration(),
            supports_config_change_registration: params.supports_config_change_registration(),
        }
    }
}
