use std::iter;

use tower_lsp::lsp_types::{Position, SemanticToken};

use crate::config::PositionEncoding;
use crate::ext::{PositionExt, StrExt};
use crate::lsp_typst_boundary::typst_to_lsp;
use crate::workspace::source::Source;

use super::Token;

pub(super) fn encode_tokens<'a>(tokens: impl Iterator<Item = Token> + 'a, source: &'a Source, encoding: PositionEncoding) -> impl Iterator<Item = SemanticToken> + 'a {
    tokens.scan(Position::new(0, 0), move |last_position, token| {
        let (encoded_tokens, position) = encode_token(token, last_position, source, encoding);
        *last_position = position;
        Some(encoded_tokens)
    }).flatten()
}

fn encode_token(token: Token, last_position: &Position, source: &Source, encoding: PositionEncoding) -> (impl Iterator<Item = SemanticToken>, Position) {
    let position =
        typst_to_lsp::offset_to_position(token.offset, encoding, source.as_ref());
    let delta = last_position.delta(&position);

    let length = token.source.as_str().encoded_len(encoding);

    let lsp_tokens = iter::once(SemanticToken {
        delta_line: delta.line,
        delta_start: delta.character,
        length: length as u32,
        token_type: token.token_type as u32,
        token_modifiers_bitset: token.modifiers.bitset(),
    });
    
    (lsp_tokens, position)
}
