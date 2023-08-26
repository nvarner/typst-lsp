use anyhow::Context;
use tower_lsp::lsp_types::{Hover, Url};
use typst::syntax::LinkedNode;
use typst::World;

use crate::lsp_typst_boundary::{lsp_to_typst, typst_to_lsp, LspPosition};

use super::TypstServer;

impl TypstServer {
    pub async fn get_hover(
        &self,
        uri: &Url,
        position: LspPosition,
    ) -> anyhow::Result<Option<Hover>> {
        let position_encoding = self.const_config().position_encoding;

        let result = self
            .thread_with_world(uri)
            .await?
            .run(move |world| {
                let source = world.main();

                let typst_offset =
                    lsp_to_typst::position_to_offset(position, position_encoding, &source);

                let typst_tooltip = typst::ide::tooltip(&world, &[], &source, typst_offset)?;

                Some((typst_offset, typst_tooltip))
            })
            .await;
        let Some((typst_offset, typst_tooltip)) = result else {
            return Ok(None);
        };

        let lsp_tooltip = typst_to_lsp::tooltip(&typst_tooltip);

        let lsp_hovered_range = self.scope_with_source(uri).await?.run(|source, _| {
            let typst_hovered_node = LinkedNode::new(source.root())
                .leaf_at(typst_offset)
                .context("")?;
            anyhow::Ok(typst_to_lsp::range(
                typst_hovered_node.range(),
                source,
                self.const_config().position_encoding,
            ))
        })?;

        Ok(Some(Hover {
            contents: lsp_tooltip,
            range: Some(lsp_hovered_range.raw_range),
        }))
    }
}
