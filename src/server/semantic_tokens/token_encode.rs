use itertools::Itertools;
use tower_lsp::lsp_types::{Position, SemanticToken};
use typst_library::prelude::EcoString;

use crate::config::PositionEncoding;
use crate::ext::{PositionDelta, PositionExt, SemanticTokenExt, StrExt};
use crate::lsp_typst_boundary::typst_to_lsp;
use crate::workspace::source::Source;

use super::Token;

pub(super) fn encode_tokens<'a>(
    tokens: impl Iterator<Item = Token> + 'a,
    source: &'a Source,
    encoding: PositionEncoding,
) -> impl Iterator<Item = (SemanticToken, EcoString)> + 'a {
    tokens.scan(Position::new(0, 0), move |last_position, token| {
        let (encoded_token, source_code, position) =
            encode_token(token, last_position, source, encoding);
        *last_position = position;
        Some((encoded_token, source_code))
    })
}

/// Splits a maybe multiline [`SemanticToken`] into single line tokens, and provides a delta from
/// the original position
pub(super) fn split_lsp_token(
    token: SemanticToken,
    delta_to_start: PositionDelta,
    source: &str,
    encoding: PositionEncoding,
) -> (impl Iterator<Item = SemanticToken>, PositionDelta) {
    let mut lines = source
        .split('\n') // we can't use `.lines()` since it drops end lines, which we need to count
        .map(|line| line.trim_end()) // remove stray '\r'. trimming start messes up `offset`
        .map(|line| line.encoded_len(encoding) as u32);

    let first = lines.next().map(|length| {
        let start = token.get_relative_position() - delta_to_start;
        token.with_relative_position(start).with_length(length)
    });
    let rest = lines.map(|length| {
        let start = PositionDelta::advance_lines(1);
        token.with_relative_position(start).with_length(length)
    });

    let tokens = first.into_iter().chain(rest).collect_vec();
    let delta = PositionDelta::advance_lines((tokens.len() - 1) as u32);
    (tokens.into_iter(), delta)
}

fn encode_token(
    token: Token,
    last_position: &Position,
    source: &Source,
    encoding: PositionEncoding,
) -> (SemanticToken, EcoString, Position) {
    let position = typst_to_lsp::offset_to_position(token.offset, encoding, source.as_ref());
    let delta = last_position.delta(&position);

    let length = token.source.as_str().encoded_len(encoding);

    let lsp_token = SemanticToken {
        delta_line: delta.delta_line,
        delta_start: delta.delta_start,
        length: length as u32,
        token_type: token.token_type as u32,
        token_modifiers_bitset: token.modifiers.bitset(),
    };

    (lsp_token, token.source, position)
}
