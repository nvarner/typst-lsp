use lazy_static::lazy_static;
use strum::{EnumIter, IntoEnumIterator};
use tower_lsp::lsp_types::{SemanticTokenType, SemanticTokensLegend};
use typst::syntax::LinkedNode;

use crate::lsp_typst_boundary::world::WorkspaceWorld;
use crate::workspace::source::Source;

use super::TypstServer;

pub const RAW: SemanticTokenType = SemanticTokenType::new("raw");

#[derive(EnumIter)]
#[repr(u32)]
pub enum TypstSemanticTokenType {
    Comment,
    String,
    Keyword,
    Operator,
    Number,
    Function,
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
        }
    }
}

lazy_static! {
    static ref LEGEND: SemanticTokensLegend = SemanticTokensLegend {
        token_types: TypstSemanticTokenType::iter().map(Into::into).collect(),
        token_modifiers: vec![],
    };
}

impl TypstServer {
    pub fn get_semantic_tokens_full(&self, world: &WorkspaceWorld, source: &Source) -> Option<()> {
        let root = LinkedNode::new(source.as_ref().root());

        todo!()
    }
}
