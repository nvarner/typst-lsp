use anyhow::{anyhow, bail, Result};
use tower_lsp::lsp_types::*;
use typst::syntax::{ast, LinkedNode, Source, SyntaxKind};

use crate::{config::PositionEncoding, lsp_typst_boundary::typst_to_lsp};

use super::TypstServer;

/// Get all symbols for a node recursively.
pub fn get_symbols(
    node: &LinkedNode,
    source: &Source,
    uri: &Url,
    query_string: Option<&str>,
    position_encoding: PositionEncoding,
) -> Result<Vec<SymbolInformation>> {
    let mut vec: Vec<SymbolInformation> = Vec::new();
    if node.children().len() > 0 {
        // recursively get identifiers for all children of the current node
        let mut children_symbols: Vec<SymbolInformation> = node
            .children()
            .map(|c| get_symbols(&c, source, uri, query_string, position_encoding))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        vec.append(&mut children_symbols);
    }
    // in case the current node is a symbol, add it to the list.
    let Some(symbol) = get_ident(node, source, uri, query_string, position_encoding)? else {
        return Ok(vec);
    };
    vec.push(symbol);
    Ok(vec)
}

/// Get symbol for a leaf node of a valid type, or `None` if the node is an invalid type.
#[allow(deprecated)]
fn get_ident(
    node: &LinkedNode,
    source: &Source,
    uri: &Url,
    query_string: Option<&str>,
    position_encoding: PositionEncoding,
) -> Result<Option<SymbolInformation>> {
    match node.kind() {
        SyntaxKind::Label => {
            let ast_node = node
                .cast::<ast::Label>()
                .ok_or_else(|| anyhow!("cast to ast node failed: {:?}", node))?;
            let name = ast_node.get().to_string();
            if let Some(query) = query_string {
                if !name.contains(query) {
                    return Ok(None);
                }
            }
            let symbol = SymbolInformation {
                name,
                kind: SymbolKind::CONSTANT,
                tags: None,
                deprecated: None, // do not use, deprecated, use `tags` instead
                location: Location {
                    uri: uri.clone(),
                    range: typst_to_lsp::range(node.range(), source, position_encoding).raw_range,
                },
                container_name: None,
            };
            Ok(Some(symbol))
        }
        SyntaxKind::Ident => {
            let ast_node = node
                .cast::<ast::Ident>()
                .ok_or_else(|| anyhow!("cast to ast node failed: {:?}", node))?;
            let name = ast_node.get().to_string();
            if let Some(query) = query_string {
                if !name.contains(query) {
                    return Ok(None);
                }
            }
            let Some(parent) = node.parent() else {
                return Ok(None);
            };
            let kind = match parent.kind() {
                // in case we have a Destructuring pattern holding an Ident, we need to check the parent of the pattern
                SyntaxKind::Destructuring => {
                    let Some(parent) = parent.parent() else {
                        return Ok(None);
                    };
                    match parent.kind() {
                        SyntaxKind::LetBinding => SymbolKind::VARIABLE,
                        SyntaxKind::Closure => SymbolKind::FUNCTION,
                        _ => return Ok(None),
                    }
                }
                _ => return Ok(None),
            };
            let symbol = SymbolInformation {
                name,
                kind,
                tags: None,
                deprecated: None, // do not use, deprecated, use `tags` instead
                location: Location {
                    uri: uri.clone(),
                    range: typst_to_lsp::range(node.range(), source, position_encoding).raw_range,
                },
                container_name: None,
            };
            Ok(Some(symbol))
        }
        SyntaxKind::Markup => {
            let name = node.get().to_owned().into_text().to_string();
            if name.is_empty() {
                return Ok(None);
            }
            if let Some(query) = query_string {
                if !name.contains(query) {
                    return Ok(None);
                }
            }
            let Some(parent) = node.parent() else {
                return Ok(None);
            };
            let kind = match parent.kind() {
                SyntaxKind::Heading => SymbolKind::NAMESPACE,
                _ => return Ok(None),
            };
            let symbol = SymbolInformation {
                name,
                kind,
                tags: None,
                deprecated: None, // do not use, deprecated, use `tags` instead
                location: Location {
                    uri: uri.clone(),
                    range: typst_to_lsp::range(node.range(), source, position_encoding).raw_range,
                },
                container_name: None,
            };
            Ok(Some(symbol))
        }
        _ => Ok(None),
    }
}

impl TypstServer {
    pub async fn get_document_symbols(
        &self,
        document: &Url,
        query_string: Option<&str>,
    ) -> Result<Vec<SymbolInformation>> {
        let config = self
            .const_config
            .get()
            .expect("const_config not initialized");
        let workspace = self.workspace.read().await;
        let Some(source_id) = workspace.sources.get_id_by_uri(document) else {
            bail!("no source found for uri: {document}");
        };
        let source = workspace.sources.get_open_source_by_id(source_id);
        let root = LinkedNode::new(source.as_ref().root());
        get_symbols(
            &root,
            source.as_ref(),
            document,
            query_string,
            config.position_encoding,
        )
    }
}
