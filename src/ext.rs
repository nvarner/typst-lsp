use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use itertools::Itertools;
use tower_lsp::lsp_types::{
    InitializeParams, Position, PositionEncodingKind, SemanticTokensClientCapabilities,
};
use typst::file::FileId;
use typst::util::StrExt as TypstStrExt;

use crate::config::PositionEncoding;

pub trait InitializeParamsExt {
    fn position_encodings(&self) -> &[PositionEncodingKind];
    fn supports_config_change_registration(&self) -> bool;
    fn semantic_tokens_capabilities(&self) -> Option<&SemanticTokensClientCapabilities>;
    fn supports_semantic_tokens_dynamic_registration(&self) -> bool;
    fn root_paths(&self) -> Vec<PathBuf>;
}

static DEFAULT_ENCODING: [PositionEncodingKind; 1] = [PositionEncodingKind::UTF16];

impl InitializeParamsExt for InitializeParams {
    fn position_encodings(&self) -> &[PositionEncodingKind] {
        self.capabilities
            .general
            .as_ref()
            .and_then(|general| general.position_encodings.as_ref())
            .map(|encodings| encodings.as_slice())
            .unwrap_or(&DEFAULT_ENCODING)
    }

    fn supports_config_change_registration(&self) -> bool {
        self.capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.configuration)
            .unwrap_or(false)
    }

    fn semantic_tokens_capabilities(&self) -> Option<&SemanticTokensClientCapabilities> {
        self.capabilities
            .text_document
            .as_ref()?
            .semantic_tokens
            .as_ref()
    }

    fn supports_semantic_tokens_dynamic_registration(&self) -> bool {
        self.semantic_tokens_capabilities()
            .and_then(|semantic_tokens| semantic_tokens.dynamic_registration)
            .unwrap_or(false)
    }

    #[allow(deprecated)] // `self.root_path` is marked as deprecated
    fn root_paths(&self) -> Vec<PathBuf> {
        match self.workspace_folders.as_ref() {
            Some(roots) => roots
                .iter()
                .map(|root| &root.uri)
                .filter_map(|uri| uri.to_file_path().ok())
                .collect_vec(),
            None => {
                let root_uri = || self.root_uri.as_ref()?.to_file_path().ok();
                let root_path = || self.root_path.as_ref()?.try_into().ok();

                root_uri().or_else(root_path).into_iter().collect()
            }
        }
    }
}

pub trait StrExt {
    fn encoded_len(&self, encoding: PositionEncoding) -> usize;
}

impl StrExt for str {
    fn encoded_len(&self, encoding: PositionEncoding) -> usize {
        match encoding {
            PositionEncoding::Utf8 => self.len(),
            PositionEncoding::Utf16 => self.len_utf16(),
        }
    }
}

pub trait PathExt {
    /// Creates a [`PathBuf`] with `self` adjoined to `prefix`. See [`PathBuf::push`] for semantics.
    fn push_front(&self, prefix: impl AsRef<Path>) -> PathBuf;

    fn root() -> &'static Path;
    fn is_typst(&self) -> bool;
}

impl PathExt for Path {
    fn push_front(&self, prefix: impl AsRef<Path>) -> PathBuf {
        prefix.as_ref().join(self)
    }

    fn root() -> &'static Path {
        Path::new("/")
    }

    fn is_typst(&self) -> bool {
        self.extension().map_or(false, |ext| ext == "typ")
    }
}

pub trait FileIdExt {
    fn with_extension(self, extension: impl AsRef<OsStr>) -> Self;
}

impl FileIdExt for FileId {
    fn with_extension(self, extension: impl AsRef<OsStr>) -> Self {
        let path = self.path().with_extension(extension);
        Self::new(self.package().cloned(), &path)
    }
}

pub trait PositionExt {
    fn delta(&self, to: &Self) -> PositionDelta;
}

impl PositionExt for Position {
    /// Calculates the delta from `self` to `to`. This is in the `SemanticToken` sense, so the
    /// delta's `character` is relative to `self`'s `character` iff `self` and `to` are on the same
    /// line. Otherwise, it's relative to the start of the line `to` is on.
    fn delta(&self, to: &Self) -> PositionDelta {
        let line_delta = to.line - self.line;
        let char_delta = if line_delta == 0 {
            to.character - self.character
        } else {
            to.character
        };

        PositionDelta {
            delta_line: line_delta,
            delta_start: char_delta,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Default)]
pub struct PositionDelta {
    pub delta_line: u32,
    pub delta_start: u32,
}
