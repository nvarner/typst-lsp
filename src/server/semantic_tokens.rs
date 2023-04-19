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

#[derive(EnumIter)]
#[repr(u32)]
pub enum TypstSemanticTokenType {
    Comment,
    String,
    Keyword,
    Operator,
    Number,
    Function,
    Decorator,
}

impl From<TypstSemanticTokenType> for SemanticTokenType {
    fn from(token_type: TypstSemanticTokenType) -> Self {
        match token_type {
            TypstSemanticTokenType::Comment => Self::COMMENT,
            TypstSemanticTokenType::String => Self::STRING,
            TypstSemanticTokenType::Keyword => Self::KEYWORD,
            TypstSemanticTokenType::Operator => Self::OPERATOR,
            TypstSemanticTokenType::Number => Self::NUMBER,
            TypstSemanticTokenType::Function => Self::FUNCTION,
            TypstSemanticTokenType::Decorator => Self::DECORATOR,
        }
    }
}

pub fn get_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TypstSemanticTokenType::iter().map(Into::into).collect(),
        token_modifiers: vec![SemanticTokenModifier::DEFAULT_LIBRARY],
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
            let token_type = highlight(node).and_then(typst_to_lsp::tag_to_token_type);
            if let Some(token_type) = token_type {
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
                    token_modifiers_bitset: 0,
                });
            }

            leaf = node.next_leaf();
        }

        Some(tokens)
    }
}
