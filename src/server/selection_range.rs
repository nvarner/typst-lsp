use super::TypstServer;
use crate::{
    config::PositionEncoding,
    lsp_typst_boundary::{lsp_to_typst, typst_to_lsp, LspPosition},
    workspace::source::Source,
};
use tower_lsp::lsp_types::SelectionRange;
use typst::syntax::LinkedNode;

fn range_for_node(
    source: &Source,
    position_encoding: PositionEncoding,
    node: &LinkedNode,
) -> SelectionRange {
    let range = typst_to_lsp::range(node.range(), source.as_ref(), position_encoding);
    SelectionRange {
        range: range.raw_range,
        parent: node
            .parent()
            .map(|node| Box::new(range_for_node(source, position_encoding, node))),
    }
}

impl TypstServer {
    pub fn get_selection_range(
        &self,
        source: &Source,
        positions: &[LspPosition],
    ) -> Option<Vec<SelectionRange>> {
        let position_encoding = self.get_const_config().position_encoding;
        let mut ranges = Vec::new();
        for &position in positions {
            let typst_offset =
                lsp_to_typst::position_to_offset(position, position_encoding, source.as_ref());
            let leaf = self.get_leaf(source, typst_offset)?;
            ranges.push(range_for_node(source, position_encoding, &leaf));
        }
        Some(ranges)
    }
}
