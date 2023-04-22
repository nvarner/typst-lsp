use std::ops;

use lazy_static::__Deref;
use strum::{EnumIter, IntoEnumIterator};
use tower_lsp::lsp_types::{
    Position, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensLegend,
};
use typst::ide::highlight;
use typst::syntax::{LinkedNode, SyntaxKind};
use typst_library::prelude::EcoString;

use crate::ext::{PositionExt, StrExt};
use crate::lsp_typst_boundary::typst_to_lsp;
use crate::workspace::source::Source;

use super::TypstServer;

const PUNCTUATION: SemanticTokenType = SemanticTokenType::new("punct");
const ESCAPE: SemanticTokenType = SemanticTokenType::new("escape");
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
const STYLED: SemanticTokenType = SemanticTokenType::new("styled");

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
    Styled,
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
            Styled => STYLED,
        }
    }
}

const STRONG: SemanticTokenModifier = SemanticTokenModifier::new("strong");
const EMPH: SemanticTokenModifier = SemanticTokenModifier::new("emph");
const MATH: SemanticTokenModifier = SemanticTokenModifier::new("math");

#[derive(Clone, Copy, EnumIter)]
#[repr(u8)]
pub enum Modifier {
    Strong,
    Emph,
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
            Strong => STRONG,
            Emph => EMPH,
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

impl ops::BitOr for ModifierSet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

pub fn get_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TokenType::iter().map(Into::into).collect(),
        token_modifiers: Modifier::iter().map(Into::into).collect(),
    }
}

struct TokenInfo {
    pub token_type: TokenType,
    pub modifiers: ModifierSet,
    pub offset: usize,
    pub source: EcoString,
}

impl TokenInfo {
    pub fn new(token_type: TokenType, modifiers: ModifierSet, node: &LinkedNode) -> Self {
        let owned_node = node.deref().clone();
        let source = owned_node.into_text();

        Self {
            token_type,
            modifiers,
            offset: node.offset(),
            source,
        }
    }
}

impl TypstServer {
    pub fn get_semantic_tokens_full(&self, source: &Source) -> Option<Vec<SemanticToken>> {
        let encoding = self.get_const_config().position_encoding;

        let mut output_tokens = Vec::new();
        let mut last_position = Position::new(0, 0);

        let root = LinkedNode::new(source.as_ref().root());

        let tokens = self.tokenize_node(&root, ModifierSet::empty());
        for token in tokens {
            let position =
                typst_to_lsp::offset_to_position(token.offset, encoding, source.as_ref());
            let delta = last_position.delta(&position);
            last_position = position;

            let length = token.source.as_str().encoded_len(encoding);

            output_tokens.push(SemanticToken {
                delta_line: delta.line,
                delta_start: delta.character,
                length: length as u32,
                token_type: token.token_type as u32,
                token_modifiers_bitset: token.modifiers.bitset(),
            });
        }

        Some(output_tokens)
    }

    fn modifiers_from_node(&self, node: &LinkedNode) -> ModifierSet {
        match node.kind() {
            SyntaxKind::Emph => ModifierSet::new(&[Modifier::Emph]),
            SyntaxKind::Strong => ModifierSet::new(&[Modifier::Strong]),
            SyntaxKind::Math => ModifierSet::new(&[Modifier::Math]),
            _ => ModifierSet::empty(),
        }
    }

    fn tokenize_single_node(&self, node: &LinkedNode, modifiers: ModifierSet) -> Option<TokenInfo> {
        highlight(node)
            .and_then(typst_to_lsp::tag_to_token)
            .map(|token_type| TokenInfo::new(token_type, modifiers, node))
    }

    fn tokenize_node<'a>(
        &'a self,
        node: &LinkedNode<'a>,
        parent_modifiers: ModifierSet,
    ) -> Box<dyn Iterator<Item = TokenInfo> + 'a> {
        let root_modifiers = self.modifiers_from_node(node);
        let modifiers = parent_modifiers | root_modifiers;

        let token = self.tokenize_single_node(node, modifiers).into_iter();
        let children = node
            .children()
            .flat_map(move |child| self.tokenize_node(&child, modifiers));
        Box::new(token.chain(children))
    }
}
