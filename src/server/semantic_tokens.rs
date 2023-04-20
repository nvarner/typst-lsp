use strum::{EnumIter, IntoEnumIterator};
use tower_lsp::lsp_types::{
    Position, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensLegend,
};
use typst::ide::highlight;
use typst::syntax::LinkedNode;
use typst::util::StrExt;

use crate::ext::PositionExt;
use crate::lsp_typst_boundary::typst_to_lsp;
use crate::workspace::source::Source;

use super::TypstServer;

const PUNCTUATION: SemanticTokenType = SemanticTokenType::new("punct");
const ESCAPE: SemanticTokenType = SemanticTokenType::new("escape");
const STRONG: SemanticTokenType = SemanticTokenType::new("strong");
const EMPH: SemanticTokenType = SemanticTokenType::new("emph");
const LINK: SemanticTokenType = SemanticTokenType::new("link");
const RAW: SemanticTokenType = SemanticTokenType::new("raw");
const LABEL: SemanticTokenType = SemanticTokenType::new("label");
const REF: SemanticTokenType = SemanticTokenType::new("ref");
const HEADING: SemanticTokenType = SemanticTokenType::new("heading");
const LIST_MARKER: SemanticTokenType = SemanticTokenType::new("marker");
const LIST_TERM: SemanticTokenType = SemanticTokenType::new("term");
const DELIMITER: SemanticTokenType = SemanticTokenType::new("delim");
const INTERPOLATED: SemanticTokenType = SemanticTokenType::new("pol");
const ERROR: SemanticTokenType = SemanticTokenType::new("error");

/// Very similar to [`typst::ide::Tag`], but with convenience traits, and extensible if we want
/// to further customize highlighting.
#[derive(Clone, Copy, EnumIter)]
#[repr(u32)]
pub enum TokenType {
    // Standard LSP types
    Comment,
    String,
    Keyword,
    Operator,
    Number,
    Function,
    Decorator,
    // Custom types
    Punctuation,
    Escape,
    Strong,
    Emph,
    Link,
    Raw,
    Label,
    Ref,
    Heading,
    ListMarker,
    ListTerm,
    Delimiter,
    Interpolated,
    Error,
}

impl From<TokenType> for SemanticTokenType {
    fn from(token_type: TokenType) -> Self {
        use TokenType::*;

        match token_type {
            Comment => Self::COMMENT,
            String => Self::STRING,
            Keyword => Self::KEYWORD,
            Operator => Self::OPERATOR,
            Number => Self::NUMBER,
            Function => Self::FUNCTION,
            Decorator => Self::DECORATOR,
            Punctuation => PUNCTUATION,
            Escape => ESCAPE,
            Strong => STRONG,
            Emph => EMPH,
            Link => LINK,
            Raw => RAW,
            Label => LABEL,
            Ref => REF,
            Heading => HEADING,
            ListMarker => LIST_MARKER,
            ListTerm => LIST_TERM,
            Delimiter => DELIMITER,
            Interpolated => INTERPOLATED,
            Error => ERROR,
        }
    }
}

const MATH: SemanticTokenModifier = SemanticTokenModifier::new("math");

#[derive(Clone, Copy, EnumIter)]
#[repr(u8)]
pub enum Modifier {
    Math,
}

impl Modifier {
    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn bitmask(self) -> u32 {
        0b1 << self.index()
    }
}

impl From<Modifier> for SemanticTokenModifier {
    fn from(modifier: Modifier) -> Self {
        use Modifier::*;

        match modifier {
            Math => MATH,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ModifierSet(u32);

impl ModifierSet {
    pub fn empty() -> Self {
        Self(0)
    }

    pub fn new(modifiers: &[Modifier]) -> Self {
        let bits = modifiers
            .iter()
            .copied()
            .map(Modifier::bitmask)
            .fold(0, |bits, mask| bits | mask);
        Self(bits)
    }

    pub fn bitset(self) -> u32 {
        self.0
    }
}

pub fn get_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TokenType::iter().map(Into::into).collect(),
        token_modifiers: Modifier::iter().map(Into::into).collect(),
    }
}

impl TypstServer {
    pub fn get_semantic_tokens_full(&self, source: &Source) -> Option<Vec<SemanticToken>> {
        let encoding = self.get_const_config().position_encoding;

        let mut tokens = Vec::new();
        let mut last_position = Position::new(0, 0);

        let root = LinkedNode::new(source.as_ref().root());
        let mut leaf = root.leftmost_leaf();

        while let Some(node) = &leaf {
            let token_type = highlight(node).map(typst_to_lsp::tag_to_token);
            if let Some((token_type, modifiers)) = token_type {
                let position =
                    typst_to_lsp::offset_to_position(node.offset(), encoding, source.as_ref());
                let delta = last_position.delta(&position);
                last_position = position;

                let length = node.text().len_utf16();

                tokens.push(SemanticToken {
                    delta_line: delta.line,
                    delta_start: delta.character,
                    length: length as u32,
                    token_type: token_type as u32,
                    token_modifiers_bitset: modifiers.bitset(),
                });
            }

            leaf = node.next_leaf();
        }

        Some(tokens)
    }
}
