use strum::IntoEnumIterator;
use tower_lsp::lsp_types::{Position, SemanticToken, SemanticTokensLegend};
use typst::syntax::{ast, LinkedNode, SyntaxKind};
use typst_library::prelude::EcoString;

use crate::ext::{PositionExt, StrExt};
use crate::lsp_typst_boundary::typst_to_lsp;
use crate::workspace::source::Source;

use self::modifier_set::ModifierSet;
use self::typst_tokens::{Modifier, TokenType};

use super::TypstServer;

mod modifier_set;
mod typst_tokens;

pub fn get_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TokenType::iter().map(Into::into).collect(),
        token_modifiers: Modifier::iter().map(Into::into).collect(),
    }
}

impl TypstServer {
    pub fn get_semantic_tokens_full(&self, source: &Source) -> Vec<SemanticToken> {
        let encoding = self.get_const_config().position_encoding;

        let mut output_tokens = Vec::new();
        let mut last_position = Position::new(0, 0);

        let root = LinkedNode::new(source.as_ref().root());

        let tokens = tokenize_tree(&root, ModifierSet::empty());
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

        output_tokens
    }
}

fn tokenize_single_node(node: &LinkedNode, modifiers: ModifierSet) -> Option<TokenInfo> {
    let is_leaf = node.children().next().is_none();

    token_from_node(node)
        .or_else(|| is_leaf.then_some(TokenType::Text))
        .map(|token_type| TokenInfo::new(token_type, modifiers, node))
}

/// Tokenize a node and its children
fn tokenize_tree<'a>(
    root: &LinkedNode<'a>,
    parent_modifiers: ModifierSet,
) -> Box<dyn Iterator<Item = TokenInfo> + 'a> {
    let modifiers = parent_modifiers | modifiers_from_node(root);

    let token = tokenize_single_node(root, modifiers).into_iter();
    let children = root
        .children()
        .flat_map(move |child| tokenize_tree(&child, modifiers));
    Box::new(token.chain(children))
}

struct TokenInfo {
    pub token_type: TokenType,
    pub modifiers: ModifierSet,
    pub offset: usize,
    pub source: EcoString,
}

impl TokenInfo {
    pub fn new(token_type: TokenType, modifiers: ModifierSet, node: &LinkedNode) -> Self {
        let source = node.get().clone().into_text();

        Self {
            token_type,
            modifiers,
            offset: node.offset(),
            source,
        }
    }
}

/// Determines the [`Modifier`]s to be applied to a node and all its children.
///
/// Note that this does not recurse up, so calling it on a child node may not return a modifier that
/// should be applied to it due to a parent.
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
