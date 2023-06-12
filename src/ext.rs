use std::ops;

use tower_lsp::lsp_types::{
    InitializeParams, Position, PositionEncodingKind, SemanticToken,
    SemanticTokensClientCapabilities,
};
use typst::util::StrExt as TypstStrExt;

use crate::config::PositionEncoding;

pub trait InitializeParamsExt {
    fn position_encodings(&self) -> &[PositionEncodingKind];
    fn semantic_tokens_capabilities(&self) -> Option<&SemanticTokensClientCapabilities>;
    fn supports_semantic_tokens_dynamic_registration(&self) -> bool;
    fn supports_multiline_tokens(&self) -> bool;
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

    fn supports_multiline_tokens(&self) -> bool {
        self.semantic_tokens_capabilities()
            .and_then(|semantic_tokens| semantic_tokens.multiline_token_support)
            .unwrap_or(false)
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

pub trait PositionExt {
    fn delta(&self, to: &Self) -> PositionDelta;
    fn advance_lines(&self, lines: usize) -> Self;
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

    fn advance_lines(&self, lines: usize) -> Self {
        let character = if lines == 0 { self.character } else { 0 };
        let line = self.line + lines as u32;
        Self { line, character }
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Default)]
pub struct PositionDelta {
    pub delta_line: u32,
    pub delta_start: u32,
}

impl PositionDelta {
    pub fn advance_lines(lines: u32) -> Self {
        Self {
            delta_line: lines,
            delta_start: 0,
        }
    }
}

impl ops::Add for PositionDelta {
    type Output = PositionDelta;

    fn add(self, rhs: Self) -> Self::Output {
        if rhs.delta_line == 0 {
            Self {
                delta_line: self.delta_line,
                delta_start: self.delta_start + rhs.delta_start,
            }
        } else {
            Self {
                delta_line: self.delta_line + rhs.delta_line,
                delta_start: rhs.delta_start,
            }
        }
    }
}

impl ops::Sub for PositionDelta {
    type Output = PositionDelta;

    fn sub(self, rhs: Self) -> Self::Output {
        let delta_line = self.delta_line - rhs.delta_line;

        if delta_line == 0 {
            // new start is on the same line as this token
            Self {
                delta_line,
                delta_start: self.delta_start - rhs.delta_start,
            }
        } else {
            Self {
                delta_line,
                delta_start: self.delta_start,
            }
        }
    }
}

pub trait SemanticTokenExt {
    /// Gets the position of the start of the token relative to the start of the last token
    fn get_relative_position(&self) -> PositionDelta;
    fn with_relative_position(&self, position: PositionDelta) -> Self;
    fn with_length(&self, length: u32) -> Self;
}

impl SemanticTokenExt for SemanticToken {
    fn get_relative_position(&self) -> PositionDelta {
        PositionDelta {
            delta_line: self.delta_line,
            delta_start: self.delta_start,
        }
    }

    fn with_relative_position(&self, position: PositionDelta) -> Self {
        Self {
            delta_line: position.delta_line,
            delta_start: position.delta_start,
            ..*self
        }
    }

    fn with_length(&self, length: u32) -> Self {
        Self { length, ..*self }
    }
}
