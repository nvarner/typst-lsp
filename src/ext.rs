use tower_lsp::lsp_types::{InitializeParams, Position, PositionEncodingKind};
use typst::util::StrExt as TypstStrExt;

use crate::config::PositionEncoding;

pub trait InitializeParamsExt {
    fn position_encodings(&self) -> &[PositionEncodingKind];
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
    fn delta(&self, to: &Self) -> Self;
}

impl PositionExt for Position {
    /// Calculates the delta from `self` to `to`. This is in the `SemanticToken` sense, so the
    /// delta's `character` is relative to `self`'s `character` iff `self` and `to` are on the same
    /// line. Otherwise, it's relative to the start of the line `to` is on.
    fn delta(&self, to: &Self) -> Self {
        let line_delta = to.line - self.line;
        let char_delta = if line_delta == 0 {
            to.character - self.character
        } else {
            to.character
        };

        Self {
            line: line_delta,
            character: char_delta,
        }
    }
}
