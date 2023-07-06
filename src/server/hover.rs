use tower_lsp::lsp_types::Hover;
use typst::ide::tooltip;
use typst::syntax::{LinkedNode, Source};

use crate::lsp_typst_boundary::world::WorkspaceWorld;
use crate::lsp_typst_boundary::{lsp_to_typst, typst_to_lsp, LspPosition};

use super::TypstServer;

impl TypstServer {
    pub fn get_hover(
        &self,
        world: &WorkspaceWorld,
        source: &Source,
        position: LspPosition,
    ) -> Option<Hover> {
        let typst_offset = lsp_to_typst::position_to_offset(
            position,
            self.get_const_config().position_encoding,
            source,
        );

        let typst_tooltip = tooltip(world, &[], source, typst_offset)?;
        let lsp_tooltip = typst_to_lsp::tooltip(&typst_tooltip);

        let typst_hovered_node = LinkedNode::new(source.root()).leaf_at(typst_offset)?;
        let lsp_hovered_range = typst_to_lsp::range(
            typst_hovered_node.range(),
            source,
            self.get_const_config().position_encoding,
        );

        Some(Hover {
            contents: lsp_tooltip,
            range: Some(lsp_hovered_range.raw_range),
        })
    }
}
