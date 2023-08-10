use itertools::Itertools;
use tower_lsp::lsp_types::{
    Documentation, MarkupContent, MarkupKind, ParameterInformation, ParameterLabel, SignatureHelp,
    SignatureInformation, Url,
};
use tracing::trace;
use typst::eval::{CastInfo, FuncInfo, Scopes, Value};
use typst::syntax::{ast, LinkedNode, Source, SyntaxKind};

use crate::ext::StrExt;
use crate::lsp_typst_boundary::{lsp_to_typst, LspCharacterOffset, LspPosition, TypstOffset};

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

            self.get_signature_info_at_offset(source, typst_offset, &scopes)
                .map(|signature| SignatureHelp {
                    signatures: vec![signature],
                    active_signature: Some(0),
                    active_parameter: None,
                })
        });

        Ok(signature)
    }

    #[tracing::instrument(skip(self, source, scopes))]
    fn get_signature_info_at_offset(
        &self,
        source: &Source,
        typst_offset: TypstOffset,
        scopes: &Scopes,
    ) -> Option<SignatureInformation> {
        let leaf = self.get_leaf(source, typst_offset)?;
        trace!("got leaf");
        let (func_ident, args) = self.get_surrounding_function(&leaf)?;
        trace!(ident = func_ident.as_str(), "got surrounding function");
        let deciding = self.get_deciding(&leaf);
        trace!(?scopes, "scope");
        let func_info = self.get_function_info(scopes, &func_ident)?;
        trace!("got func info");
        let current_param_index = self.get_current_param_index(&deciding, func_info, args);

        let (label, params) = self.get_param_information(func_info);

        trace!(?current_param_index, label, ?params, "got signature info");

        Some(SignatureInformation {
            label,
            documentation: Some(self.markdown_docs(func_info.docs)),
            parameters: Some(params),
            active_parameter: current_param_index.map(|i| i as u32),
        })
    }

    pub fn get_leaf<'a>(
        &self,
        source: &'a Source,
        typst_offset: TypstOffset,
    ) -> Option<LinkedNode<'a>> {
        LinkedNode::new(source.root()).leaf_at(typst_offset)
    }

    pub fn get_surrounding_function(&self, leaf: &LinkedNode) -> Option<(ast::Ident, ast::Args)> {
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

    pub fn get_function_info<'a>(
        &self,
        scopes: &'a Scopes,
        ident: &ast::Ident,
    ) -> Option<&'a FuncInfo> {
        match scopes.get(ident.as_str()) {
            Ok(Value::Func(function)) => function.info(),
            _ => None,
        }
    }

    /// Find the piece of syntax that decides what we're completing.
    pub fn get_deciding<'a>(&self, leaf: &'a LinkedNode) -> LinkedNode<'a> {
        let mut deciding = leaf.clone();
        while !matches!(
            deciding.kind(),
            SyntaxKind::LeftParen | SyntaxKind::Comma | SyntaxKind::Colon
        ) {
            let Some(prev) = deciding.prev_leaf() else { break };
            deciding = prev;
        }
        deciding
    }

    pub fn get_current_param_index(
        &self,
        deciding: &LinkedNode,
        function_info: &FuncInfo,
        args: ast::Args,
    ) -> Option<usize> {
        match deciding.kind() {
            // After colon: "func(param:|)", "func(param: |)".
            SyntaxKind::Colon => deciding
                .prev_leaf()
                .and_then(|prev| prev.cast::<ast::Ident>())
                .and_then(|param_ident| {
                    function_info
                        .params
                        .iter()
                        .position(|param| param.name == param_ident.as_str())
                }),
            // Before: "func(|)", "func(hi|)", "func(12,|)".
            SyntaxKind::Comma | SyntaxKind::LeftParen => {
                let following_param = deciding
                    .next_leaf()
                    .and_then(|next| next.cast::<ast::Ident>());
                match following_param {
                    Some(next) => function_info
                        .params
                        .iter()
                        .position(|param| param.named && param.name.starts_with(next.as_str())),
                    None => {
                        let positional_args_so_far = args
                            .items()
                            .filter(|arg| matches!(arg, ast::Arg::Pos(_)))
                            .count();
                        function_info
                            .params
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

    fn format_cast_info(info: &CastInfo) -> String {
        match info {
            CastInfo::Any => "any".to_owned(),
            CastInfo::Value(value, _) => value.repr().to_string(),
            CastInfo::Type(ty) => (*ty).to_owned(),
            CastInfo::Union(options) => options.iter().map(Self::format_cast_info).join(" "),
        }
    }

    /// Returns the signature label as well as parameter offsets of the function
    pub fn get_param_information(&self, info: &FuncInfo) -> (String, Vec<ParameterInformation>) {
        let encoding = self.const_config().position_encoding;

        let label_start = format!("{}(", info.name);
        let param_joiner = ", ";
        let param_joiner_len = param_joiner.encoded_len(encoding);

        let labels = info
            .params
            .iter()
            .map(|param| {
                let type_string = Self::format_cast_info(&param.cast);
                format!("{}: {}", param.name, type_string)
            })
            .collect::<Vec<_>>();

        let params = labels
            .iter()
            .scan(
                label_start.encoded_len(encoding),
                |start_of_label, label| {
                    let len = label.encoded_len(encoding);
                    let end_of_label = *start_of_label + len;
                    let offsets = [
                        *start_of_label as LspCharacterOffset,
                        end_of_label as LspCharacterOffset,
                    ];

                    *start_of_label += len + param_joiner_len;

                    Some(offsets)
                },
            )
            .zip(info.params.iter())
            .map(|(offsets, param)| ParameterInformation {
                label: ParameterLabel::LabelOffsets(offsets),
                documentation: Some(self.markdown_docs(param.docs)),
            })
            .collect();

        let params_label = labels.iter().join(param_joiner);

        let type_label = Self::format_cast_info(&info.returns);
        let returns_label = if !type_label.is_empty() {
            format!(" -> {type_label}")
        } else {
            "".to_owned()
        };

        let label = format!("{label_start}{params_label}){returns_label}");

        (label, params)
    }

    pub fn markdown_docs(&self, docs: &str) -> Documentation {
        Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: docs.into(),
        })
    }
}
