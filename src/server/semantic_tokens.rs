use std::ops;

use lazy_static::__Deref;
use strum::{EnumIter, IntoEnumIterator};
use tower_lsp::lsp_types::{
    Position, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensLegend,
};
use typst::syntax::{ast, LinkedNode, SyntaxKind};
use typst_library::prelude::EcoString;

use crate::ext::{PositionExt, StrExt};
use crate::lsp_typst_boundary::typst_to_lsp;
use crate::workspace::source::Source;

use super::TypstServer;

const BOOL: SemanticTokenType = SemanticTokenType::new("bool");
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
const TEXT: SemanticTokenType = SemanticTokenType::new("text");

/// Very similar to [`typst::ide::Tag`], but with convenience traits, and extensible because we want
/// to further customize highlighting
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
    Bool,
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
    /// Any text in markup without a more specific token type, possible styled.
    ///
    /// We perform styling (like bold and italics) via modifiers. That means everything that should
    /// receive styling needs to be a token so we can apply a modifier to it. This token type is
    /// mostly for that, since text should usually not be specially styled.
    Text,
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
            Bool => BOOL,
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
            Text => TEXT,
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

        let tokens = tokenize_node(&root, ModifierSet::empty());
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
}

fn modifiers_from_node(node: &LinkedNode) -> ModifierSet {
    match node.kind() {
        SyntaxKind::Emph => ModifierSet::new(&[Modifier::Emph]),
        SyntaxKind::Strong => ModifierSet::new(&[Modifier::Strong]),
        SyntaxKind::Math | SyntaxKind::Equation => ModifierSet::new(&[Modifier::Math]),
        _ => ModifierSet::empty(),
    }
}

/// Determines the best [`TokenType`] for an entire node and its children, if any. If there is no
/// single `TokenType`, or none better than `Text`, returns `None`.
///
/// In tokenization, returning `Some` stops recursion, while returning `None` continues and attempts
/// to tokenize each of `node`'s children. If there are no children, `Text` is taken as the default.
fn token_from_node(node: &LinkedNode) -> Option<TokenType> {
    use SyntaxKind::*;

    match node.kind() {
        Star if node.parent_kind() == Some(Strong) => Some(TokenType::Punctuation),
        Star if node.parent_kind() == Some(ModuleImport) => Some(TokenType::Operator),

        Underscore if node.parent_kind() == Some(Emph) => Some(TokenType::Punctuation),
        Underscore if node.parent_kind() == Some(MathAttach) => Some(TokenType::Operator),

        MathIdent | Ident => Some(token_from_ident(node)),
        Hashtag => token_from_hashtag(node),

        LeftBrace | RightBrace | LeftBracket | RightBracket | LeftParen | RightParen | Comma
        | Semicolon | Colon => Some(TokenType::Punctuation),
        Linebreak | Escape | Shorthand => Some(TokenType::Escape),
        Link => Some(TokenType::Link),
        Raw => Some(TokenType::Raw),
        Label => Some(TokenType::Label),
        RefMarker => Some(TokenType::Ref),
        Heading | HeadingMarker => Some(TokenType::Heading),
        ListMarker | EnumMarker | TermMarker => Some(TokenType::ListMarker),
        MathAlignPoint | Plus | Minus | Slash | Hat | Dot | Eq | EqEq | ExclEq | Lt | LtEq | Gt
        | GtEq | PlusEq | HyphEq | StarEq | SlashEq | Dots | Arrow | Not | And | Or | Unary
        | Binary => Some(TokenType::Operator),
        Dollar => Some(TokenType::Delimiter),
        None | Auto | Let | Show | If | Else | For | In | While | Break | Continue | Return
        | Import | Include | As | Set => Some(TokenType::Keyword),
        Bool => Some(TokenType::Bool),
        Int | Float | Numeric => Some(TokenType::Number),
        Str => Some(TokenType::String),
        LineComment | BlockComment => Some(TokenType::Comment),
        Error => Some(TokenType::Error),

        // Disambiguate from `SyntaxKind::None`
        _ => Option::None,
    }
}

// TODO: differentiate also using tokens in scope, not just context
fn is_function_ident(ident: &LinkedNode) -> bool {
    let Some(next) = ident.next_leaf() else { return false; };
    let function_call = matches!(next.kind(), SyntaxKind::LeftParen)
        && matches!(
            next.parent_kind(),
            Some(SyntaxKind::Args | SyntaxKind::Params)
        );
    let function_content = matches!(next.kind(), SyntaxKind::LeftBracket)
        && matches!(next.parent_kind(), Some(SyntaxKind::ContentBlock));
    function_call || function_content
}

fn token_from_ident(ident: &LinkedNode) -> TokenType {
    if is_function_ident(ident) {
        TokenType::Function
    } else {
        TokenType::Interpolated
    }
}

fn get_expr_following_hashtag<'a>(hashtag: &LinkedNode<'a>) -> Option<LinkedNode<'a>> {
    hashtag
        .next_sibling()
        .filter(|next| {
            next.cast::<ast::Expr>()
                .map_or(false, |expr| expr.hashtag())
        })
        .and_then(|node| node.leftmost_leaf())
}

fn token_from_hashtag(hashtag: &LinkedNode) -> Option<TokenType> {
    get_expr_following_hashtag(hashtag)
        .as_ref()
        .and_then(token_from_node)
}

fn tokenize_single_node(node: &LinkedNode, modifiers: ModifierSet) -> Option<TokenInfo> {
    let is_leaf = node.children().next().is_none();

    token_from_node(node)
        .or_else(|| is_leaf.then_some(TokenType::Text))
        .map(|token_type| TokenInfo::new(token_type, modifiers, node))
}

fn tokenize_node<'a>(
    node: &LinkedNode<'a>,
    parent_modifiers: ModifierSet,
) -> Box<dyn Iterator<Item = TokenInfo> + 'a> {
    let modifiers = parent_modifiers | modifiers_from_node(node);

    let token = tokenize_single_node(node, modifiers).into_iter();
    let children = node
        .children()
        .flat_map(move |child| tokenize_node(&child, modifiers));
    Box::new(token.chain(children))
}
