use itertools::Itertools;
use tower_lsp::lsp_types::{
    Documentation, MarkupContent, MarkupKind, ParameterInformation, SignatureHelp,
    SignatureInformation, Url,
};
use tracing::trace;
use typst::foundations::{Func, ParamInfo, Scopes, Value};
use typst::syntax::{ast, LinkedNode, Source, SyntaxKind};

use crate::lsp_typst_boundary::{lsp_to_typst, typst_to_lsp, LspPosition, TypstOffset};

use super::TypstServer;

impl TypstServer {
    pub async fn get_signature_at_position(
        &self,
        uri: &Url,
        position: LspPosition,
    ) -> anyhow::Result<Option<SignatureHelp>> {
        // TODO: This isn't the complete stack of scopes, but there doesn't seem to be a way to get
        // it from Typst. Needs investigation, possibly a PR to Typst.
        let mut scopes = self.typst_global_scopes();
        if let Some(module) = self.eval_source(uri).await?.0 {
            scopes.top = module.scope().clone();
        };

        let signature = self.scope_with_source(uri).await?.run(|source, _| {
            let typst_offset = lsp_to_typst::position_to_offset(
                position,
                self.const_config().position_encoding,
                source,
            );

            get_signature_info_at_offset(source, typst_offset, &scopes).map(|signature| {
                SignatureHelp {
                    signatures: vec![signature],
                    active_signature: Some(0),
                    active_parameter: None,
                }
            })
        });

        Ok(signature)
    }
}

#[tracing::instrument(skip(scopes))]
fn get_signature_info_at_offset(
    source: &Source,
    typst_offset: TypstOffset,
    scopes: &Scopes,
) -> Option<SignatureInformation> {
    let param_in_function = ParamInFunction::at_offset(source, typst_offset, scopes)?;
    trace!(?param_in_function, "got param in function");

    let label = param_in_function.label().to_string();
    let params = param_in_function.param_infos();
    trace!(label, ?params, "got signature info");

    let documentation = param_in_function.docs();

    let active_parameter = param_in_function.param_index().map(|i| i as u32);

    Some(SignatureInformation {
        label,
        documentation,
        parameters: Some(params),
        active_parameter,
    })
}

#[derive(Debug, Clone)]
struct ParamInFunction<'a> {
    function: &'a Func,
    param_index: Option<usize>,
}

impl<'a> ParamInFunction<'a> {
    #[tracing::instrument(skip(scopes), ret)]
    pub fn at_offset(
        source: &Source,
        typst_offset: TypstOffset,
        scopes: &'a Scopes,
    ) -> Option<Self> {
        let tree = LinkedNode::new(source.root());
        let leaf = tree.leaf_at(typst_offset)?;
        trace!("got leaf");

        Self::at_leaf(&leaf, scopes)
    }

    fn at_leaf(leaf: &LinkedNode, scopes: &'a Scopes) -> Option<Self> {
        let (ident, args) = Self::surrounding_function_syntax(leaf)?;
        let function = Self::function_value(scopes, &ident)?;
        trace!(?function, "got function");

        let param_index = Self::param_index_at_leaf(leaf, function, args);

        Some(Self {
            function,
            param_index,
        })
    }

    fn param_index_at_leaf(leaf: &LinkedNode, function: &Func, args: ast::Args) -> Option<usize> {
        let deciding = Self::deciding_syntax(leaf);
        let params = function.params()?;
        let param_index = Self::find_param_index(&deciding, params, args)?;
        trace!(param_index, "got param index");
        Some(param_index)
    }

    fn surrounding_function_syntax<'b>(
        leaf: &'b LinkedNode,
    ) -> Option<(ast::Ident<'b>, ast::Args<'b>)> {
        let parent = leaf.parent()?;
        let parent = match parent.kind() {
            SyntaxKind::Named => parent.parent()?,
            _ => parent,
        };
        let args = parent.cast::<ast::Args>()?;
        let grand = parent.parent()?;
        let expr = grand.cast::<ast::Expr>()?;
        let callee = match expr {
            ast::Expr::FuncCall(call) => call.callee(),
            ast::Expr::Set(set) => set.target(),
            _ => return None,
        };
        let callee = match callee {
            ast::Expr::Ident(callee) => callee,
            _ => return None,
        };

        Some((callee, args))
    }

    fn function_value<'b>(scopes: &'b Scopes, ident: &ast::Ident) -> Option<&'b Func> {
        match scopes.get(ident.as_str()) {
            Ok(Value::Func(function)) => Some(function),
            _ => None,
        }
    }

    /// Find the piece of syntax that decides what we're completing.
    fn deciding_syntax<'b>(leaf: &'b LinkedNode) -> LinkedNode<'b> {
        let mut deciding = leaf.clone();
        while !matches!(
            deciding.kind(),
            SyntaxKind::LeftParen | SyntaxKind::Comma | SyntaxKind::Colon
        ) {
            let Some(prev) = deciding.prev_leaf() else {
                break;
            };
            deciding = prev;
        }
        deciding
    }

    fn find_param_index(
        deciding: &LinkedNode,
        params: &[ParamInfo],
        args: ast::Args,
    ) -> Option<usize> {
        match deciding.kind() {
            // After colon: "func(param:|)", "func(param: |)".
            SyntaxKind::Colon => {
                let prev = deciding.prev_leaf()?;
                let param_ident = prev.cast::<ast::Ident>()?;
                params
                    .iter()
                    .position(|param| param.name == param_ident.as_str())
            }
            // Before: "func(|)", "func(hi|)", "func(12,|)".
            SyntaxKind::Comma | SyntaxKind::LeftParen => {
                let next = deciding.next_leaf();
                let following_param = next.as_ref().and_then(|next| next.cast::<ast::Ident>());
                match following_param {
                    Some(next) => params
                        .iter()
                        .position(|param| param.named && param.name.starts_with(next.as_str())),
                    None => {
                        let positional_args_so_far = args
                            .items()
                            .filter(|arg| matches!(arg, ast::Arg::Pos(_)))
                            .count();
                        params
                            .iter()
                            .enumerate()
                            .filter(|(_, param)| param.positional)
                            .map(|(i, _)| i)
                            .nth(positional_args_so_far)
                    }
                }
            }
            _ => None,
        }
    }

    pub fn function_name(&self) -> &str {
        self.function.name().unwrap_or("<anonymous closure>")
    }

    pub fn param_index(&self) -> Option<usize> {
        self.param_index.as_ref().copied()
    }

    pub fn docs(&self) -> Option<Documentation> {
        self.function.docs().map(Self::markdown_docs)
    }

    fn markdown_docs(docs: &str) -> Documentation {
        Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: docs.to_owned(),
        })
    }

    pub fn label(&self) -> String {
        format!(
            "{}({}){}",
            self.function_name(),
            self.param_label(),
            self.return_label()
        )
    }

    fn param_label(&self) -> String {
        match self.function.params() {
            Some(params) => params
                .iter()
                .map(typst_to_lsp::param_info_to_label)
                .join(", "),
            None => "".to_owned(),
        }
    }

    fn return_label(&self) -> String {
        match self.function.returns() {
            Some(returns) => format!("-> {}", typst_to_lsp::cast_info_to_label(returns)),
            None => "".to_owned(),
        }
    }

    pub fn param_infos(&self) -> Vec<ParameterInformation> {
        self.function
            .params()
            .unwrap_or_default()
            .iter()
            .map(typst_to_lsp::param_info)
            .collect()
    }
}
