use anyhow::{anyhow, Result};
use tower_lsp::lsp_types::*;
use typst::syntax::{ast, LinkedNode, Source, SyntaxKind};

use crate::{config::PositionEncoding, lsp_typst_boundary::typst_to_lsp};

use super::TypstServer;

/// Get all symbols for a node recursively.
pub fn get_symbols<'a>(
    node: LinkedNode<'a>,
    source: &'a Source,
    uri: &'a Url,
    query_string: Option<&'a str>,
    position_encoding: PositionEncoding,
) -> Box<dyn Iterator<Item = Result<SymbolInformation>> + 'a> {
    let own_symbol = get_ident(&node, source, uri, query_string, position_encoding).transpose();
    let children_symbols = node
        .children()
        .flat_map(move |child| get_symbols(child, source, uri, query_string, position_encoding));
    Box::new(children_symbols.chain(own_symbol))
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
                // for variable definitions, the Let binding holds an Ident
                SyntaxKind::LetBinding => SymbolKind::VARIABLE,
                // for function definitions, the Let binding holds a Closure which holds the Ident
                SyntaxKind::Closure => {
                    let Some(grand_parent) = parent.parent() else {
                        return Ok(None);
                    };
                    match grand_parent.kind() {
                        SyntaxKind::LetBinding => SymbolKind::FUNCTION,
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
    pub fn get_document_symbols<'a>(
        &'a self,
        source: &'a Source,
        uri: &'a Url,
        query_string: Option<&'a str>,
    ) -> impl Iterator<Item = Result<SymbolInformation>> + 'a {
        let const_config = self.const_config();

        let root = LinkedNode::new(source.root());
        get_symbols(
            root,
            source,
            uri,
            query_string,
            const_config.position_encoding,
        )
    }
}
