use tower_lsp::lsp_types;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportPdfMode {
    Never,
    #[default]
    OnSave,
    OnType,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub export_pdf: ExportPdfMode,
    pub position_encoding: PositionEncoding,
}
