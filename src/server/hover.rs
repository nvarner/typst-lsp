use tower_lsp::lsp_types::Hover;
use typst::syntax::LinkedNode;

use crate::lsp_typst_boundary::workaround::ide::tooltip::tooltip;
use crate::lsp_typst_boundary::{lsp_to_typst, typst_to_lsp, LspPosition};
use crate::workspace::source::Source;
use crate::workspace::Workspace;

use super::TypstServer;

impl TypstServer {
    pub fn get_hover(
        &self,
        workspace: &Workspace,
        source: &Source,
        position: LspPosition,
    ) -> Option<Hover> {
        let typst_offset = lsp_to_typst::position_to_offset(
            position,
            self.get_const_config().position_encoding,
            source,
        );

        let typst_tooltip = tooltip(workspace, &[], source.as_ref(), typst_offset)?;
        let lsp_tooltip = typst_to_lsp::tooltip(&typst_tooltip);

        let typst_hovered_node = LinkedNode::new(source.as_ref().root()).leaf_at(typst_offset)?;
        let lsp_hovered_range = typst_to_lsp::range(
            typst_hovered_node.range(),
            source.as_ref(),
            self.get_const_config().position_encoding,
        );

        Some(Hover {
            contents: lsp_tooltip,
            range: Some(lsp_hovered_range.raw_range),
        })
    }
}
