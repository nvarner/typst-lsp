//! Conversions between Typst and LSP types and representations

use tower_lsp::lsp_types;
use typst::syntax::Source;

pub type LspUri = lsp_types::Url;

pub type LspPosition = lsp_types::Position;
/// The interpretation of an `LspCharacterOffset` depends on the `LspPositionEncoding`
pub type LspCharacterOffset = u32;
pub type LspPositionEncoding = crate::config::PositionEncoding;
/// Byte offset (i.e. UTF-8 bytes) in Typst files, either from the start of the line or the file
pub type TypstOffset = usize;
pub type TypstSpan = typst::syntax::Span;

/// An LSP range. It needs its associated `LspPositionEncoding` to be used. The `LspRange` struct
/// provides this range with that encoding.
pub type LspRawRange = lsp_types::Range;
pub type TypstRange = std::ops::Range<usize>;

pub type TypstTooltip = typst_ide::Tooltip;
pub type LspHoverContents = lsp_types::HoverContents;

pub type TypstDatetime = typst::eval::Datetime;

pub type LspDiagnostic = lsp_types::Diagnostic;
pub type TypstDiagnostic = typst::diag::SourceDiagnostic;

pub type LspSeverity = lsp_types::DiagnosticSeverity;
pub type TypstSeverity = typst::diag::Severity;

pub type LspParamInfo = lsp_types::ParameterInformation;
pub type TypstParamInfo = typst::eval::ParamInfo;

/// An LSP range with its associated encoding.
pub struct LspRange {
    pub raw_range: LspRawRange,
    pub encoding: LspPositionEncoding,
}

impl LspRange {
    pub fn new(raw_range: LspRawRange, encoding: LspPositionEncoding) -> Self {
        Self {
            raw_range,
            encoding,
        }
    }

    pub fn into_range_on(self, source: &Source) -> TypstRange {
        lsp_to_typst::range(&self, source)
    }
}

pub type LspCompletion = lsp_types::CompletionItem;
pub type LspCompletionKind = lsp_types::CompletionItemKind;
pub type TypstCompletion = typst_ide::Completion;
pub type TypstCompletionKind = typst_ide::CompletionKind;

pub mod lsp_to_typst {
    use typst::syntax::Source;

    use super::*;

    pub fn position_to_offset(
        lsp_position: LspPosition,
        lsp_position_encoding: LspPositionEncoding,
        typst_source: &Source,
    ) -> TypstOffset {
        match lsp_position_encoding {
            LspPositionEncoding::Utf8 => {
                let line_index = lsp_position.line as usize;
                let column_index = lsp_position.character as usize;
                typst_source
                    .line_column_to_byte(line_index, column_index)
                    .unwrap()
            }
            LspPositionEncoding::Utf16 => {
                // We have a line number and a UTF-16 offset into that line. We want a byte offset into
                // the file.
                //
                // Typst's `Source` provides several UTF-16 methods:
                //  - `len_utf16` for the length of the file
                //  - `byte_to_utf16` to convert a byte offset from the start of the file to a UTF-16
                //       offset from the start of the file
                //  - `utf16_to_byte` to do the opposite of `byte_to_utf16`
                //
                // Unfortunately, none of these address our needs well, so we do some math instead. This
                // is not the fastest possible implementation, but it's the most reasonable without
                // access to the internal state of `Source`.

                // TODO: Typst's `Source` could easily provide an implementation of the method we need
                //   here. Submit a PR against `typst` to add it, then update this if/when merged.

                let line_index = lsp_position.line as usize;
                let utf16_offset_in_line = lsp_position.character as usize;

                let byte_line_offset = typst_source.line_to_byte(line_index).unwrap();
                let utf16_line_offset = typst_source.byte_to_utf16(byte_line_offset).unwrap();
                let utf16_offset = utf16_line_offset + utf16_offset_in_line;

                typst_source.utf16_to_byte(utf16_offset).unwrap()
            }
        }
    }

    pub fn range(lsp_range: &LspRange, source: &Source) -> TypstRange {
        let lsp_start = lsp_range.raw_range.start;
        let typst_start = position_to_offset(lsp_start, lsp_range.encoding, source);

        let lsp_end = lsp_range.raw_range.end;
        let typst_end = position_to_offset(lsp_end, lsp_range.encoding, source);

        TypstRange {
            start: typst_start,
            end: typst_end,
        }
    }
}

pub mod typst_to_lsp {
    use std::iter;

    use anyhow::bail;
    use futures::{future, stream, StreamExt, TryFutureExt, TryStreamExt};
    use itertools::{Format, Itertools};
    use lazy_static::lazy_static;
    use regex::{Captures, Regex};
    use tower_lsp::lsp_types::{
        DiagnosticRelatedInformation, Documentation, InsertTextFormat, LanguageString, Location,
        MarkedString, MarkupContent, MarkupKind,
    };
    use tracing::error;
    use typst::diag::Tracepoint;
    use typst::eval::{CastInfo, Repr};
    use typst::syntax::{FileId, Source, Spanned};
    use typst_library::prelude::EcoString;

    use crate::config::ConstConfig;
    use crate::server::diagnostics::DiagnosticsMap;
    use crate::workspace::project::Project;

    use super::*;

    pub fn offset_to_position(
        typst_offset: TypstOffset,
        lsp_position_encoding: LspPositionEncoding,
        typst_source: &Source,
    ) -> LspPosition {
        let line_index = typst_source.byte_to_line(typst_offset).unwrap();
        let column_index = typst_source.byte_to_column(typst_offset).unwrap();

        let lsp_line = line_index as u32;
        let lsp_column = match lsp_position_encoding {
            LspPositionEncoding::Utf8 => column_index as LspCharacterOffset,
            LspPositionEncoding::Utf16 => {
                // See the implementation of `lsp_to_typst::position_to_offset` for discussion
                // relevant to this function.

                // TODO: Typst's `Source` could easily provide an implementation of the method we
                //   need here. Submit a PR to `typst` to add it, then update this if/when merged.

                let utf16_offset = typst_source.byte_to_utf16(typst_offset).unwrap();

                let byte_line_offset = typst_source.line_to_byte(line_index).unwrap();
                let utf16_line_offset = typst_source.byte_to_utf16(byte_line_offset).unwrap();

                let utf16_column_offset = utf16_offset - utf16_line_offset;
                utf16_column_offset as LspCharacterOffset
            }
        };

        LspPosition::new(lsp_line, lsp_column)
    }

    pub fn range(
        typst_range: TypstRange,
        typst_source: &Source,
        lsp_position_encoding: LspPositionEncoding,
    ) -> LspRange {
        let typst_start = typst_range.start;
        let lsp_start = offset_to_position(typst_start, lsp_position_encoding, typst_source);

        let typst_end = typst_range.end;
        let lsp_end = offset_to_position(typst_end, lsp_position_encoding, typst_source);

        let raw_range = LspRawRange::new(lsp_start, lsp_end);
        LspRange::new(raw_range, lsp_position_encoding)
    }

    fn completion_kind(typst_completion_kind: TypstCompletionKind) -> LspCompletionKind {
        match typst_completion_kind {
            TypstCompletionKind::Syntax => LspCompletionKind::SNIPPET,
            TypstCompletionKind::Func => LspCompletionKind::FUNCTION,
            TypstCompletionKind::Param => LspCompletionKind::VARIABLE,
            TypstCompletionKind::Constant => LspCompletionKind::CONSTANT,
            TypstCompletionKind::Symbol(_) => LspCompletionKind::TEXT,
            TypstCompletionKind::Type => LspCompletionKind::CLASS,
        }
    }

    lazy_static! {
        static ref TYPST_SNIPPET_PLACEHOLDER_RE: Regex = Regex::new(r"\$\{(.*?)\}").unwrap();
    }

    /// Adds numbering to placeholders in snippets
    fn snippet(typst_snippet: &EcoString) -> String {
        let mut counter = 1;
        let result =
            TYPST_SNIPPET_PLACEHOLDER_RE.replace_all(typst_snippet.as_str(), |cap: &Captures| {
                // let substitution = format!("${{{}:{}}}", counter, &cap[1]);
                let substitution = format!("${counter}");
                counter += 1;
                substitution
            });

        result.to_string()
    }

    pub fn completion(typst_completion: &TypstCompletion) -> LspCompletion {
        // TODO: provide `text_edit` instead of `insert_text` as recommended by the LSP spec
        LspCompletion {
            label: typst_completion.label.to_string(),
            kind: Some(completion_kind(typst_completion.kind.clone())),
            detail: typst_completion.detail.as_ref().map(String::from),
            insert_text: typst_completion.apply.as_ref().map(snippet),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        }
    }

    pub fn completions(typst_completions: &[TypstCompletion]) -> Vec<LspCompletion> {
        typst_completions.iter().map(completion).collect_vec()
    }

    async fn tracepoint_to_relatedinformation(
        project: &Project,
        tracepoint: &Spanned<Tracepoint>,
        const_config: &ConstConfig,
    ) -> anyhow::Result<Option<DiagnosticRelatedInformation>> {
        if let Some(id) = tracepoint.span.id() {
            let full_id = project.fill_id(id);
            let uri = project.full_id_to_uri(full_id).await?;
            let source = project.read_source_by_uri(&uri)?;

            if let Some(typst_range) = source.range(tracepoint.span) {
                let lsp_range =
                    typst_to_lsp::range(typst_range, &source, const_config.position_encoding);

                return Ok(Some(DiagnosticRelatedInformation {
                    location: Location {
                        uri,
                        range: lsp_range.raw_range,
                    },
                    message: tracepoint.v.to_string(),
                }));
            }
        }

        Ok(None)
    }

    async fn diagnostic_related_information(
        project: &Project,
        typst_diagnostic: &TypstDiagnostic,
        const_config: &ConstConfig,
    ) -> anyhow::Result<Vec<DiagnosticRelatedInformation>> {
        let mut tracepoints = vec![];

        for tracepoint in &typst_diagnostic.trace {
            if let Some(info) =
                tracepoint_to_relatedinformation(project, tracepoint, const_config).await?
            {
                tracepoints.push(info);
            }
        }

        Ok(tracepoints)
    }

    async fn diagnostic(
        project: &Project,
        typst_diagnostic: &TypstDiagnostic,
        const_config: &ConstConfig,
    ) -> anyhow::Result<(LspUri, LspDiagnostic)> {
        let Some((id, span)) = diagnostic_span_id(typst_diagnostic) else {
            bail!("could not find any id")
        };
        let full_id = project.fill_id(id);
        let uri = project.full_id_to_uri(full_id).await?;

        let source = project.read_source_by_uri(&uri)?;
        let lsp_range = diagnostic_range(&source, span, const_config);

        let lsp_severity = diagnostic_severity(typst_diagnostic.severity);

        let typst_message = &typst_diagnostic.message;
        let typst_hints = &typst_diagnostic.hints;
        let lsp_message = format!("{typst_message}{}", diagnostic_hints(typst_hints));

        let tracepoints =
            diagnostic_related_information(project, typst_diagnostic, const_config).await?;

        let diagnostic = LspDiagnostic {
            range: lsp_range.raw_range,
            severity: Some(lsp_severity),
            message: lsp_message,
            source: Some("typst".to_owned()),
            related_information: Some(tracepoints),
            ..Default::default()
        };

        Ok((uri, diagnostic))
    }

    fn diagnostic_span_id(typst_diagnostic: &TypstDiagnostic) -> Option<(FileId, TypstSpan)> {
        iter::once(typst_diagnostic.span)
            .chain(typst_diagnostic.trace.iter().map(|trace| trace.span))
            .find_map(|span| Some((span.id()?, span)))
    }

    fn diagnostic_range(
        source: &Source,
        typst_span: TypstSpan,
        const_config: &ConstConfig,
    ) -> LspRange {
        // Due to #241 and maybe typst/typst#2035, we sometimes fail to find the span. In that case,
        // we use a default span as a better alternative to panicking.
        //
        // This may have been fixed after Typst 0.7.0, but it's still nice to avoid panics in case
        // something similar reappears.
        match source.find(typst_span) {
            Some(node) => {
                let typst_range = node.range();
                range(typst_range, source, const_config.position_encoding)
            }
            None => LspRange::new(
                LspRawRange::new(LspPosition::new(0, 0), LspPosition::new(0, 0)),
                const_config.position_encoding,
            ),
        }
    }

    fn diagnostic_severity(typst_severity: TypstSeverity) -> LspSeverity {
        match typst_severity {
            TypstSeverity::Error => LspSeverity::ERROR,
            TypstSeverity::Warning => LspSeverity::WARNING,
        }
    }

    fn diagnostic_hints(typst_hints: &[EcoString]) -> Format<impl Iterator<Item = EcoString> + '_> {
        iter::repeat(EcoString::from("\n\nHint: "))
            .take(typst_hints.len())
            .interleave(typst_hints.iter().cloned())
            .format("")
    }

    pub async fn diagnostics<'a>(
        project: &Project,
        errors: impl IntoIterator<Item = &'a TypstDiagnostic>,
        const_config: &ConstConfig,
    ) -> DiagnosticsMap {
        stream::iter(errors)
            .then(|error| {
                diagnostic(project, error, const_config)
                    .map_err(move |conversion_err| (conversion_err, error))
            })
            .inspect_err(|(conversion_err, typst_err)| error!(%conversion_err, ?typst_err, "could not convert Typst error to diagnostic"))
            .filter_map(|result| future::ready(result.ok()))
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .into_group_map()
    }

    pub fn tooltip(typst_tooltip: &TypstTooltip) -> LspHoverContents {
        let lsp_marked_string = match typst_tooltip {
            TypstTooltip::Text(text) => MarkedString::String(text.to_string()),
            TypstTooltip::Code(code) => MarkedString::LanguageString(LanguageString {
                language: "typst".to_owned(),
                value: code.to_string(),
            }),
        };
        LspHoverContents::Scalar(lsp_marked_string)
    }

    pub fn param_info(typst_param_info: &TypstParamInfo) -> LspParamInfo {
        LspParamInfo {
            label: lsp_types::ParameterLabel::Simple(typst_param_info.name.to_owned()),
            documentation: param_info_to_docs(typst_param_info),
        }
    }

    pub fn param_info_to_label(typst_param_info: &TypstParamInfo) -> String {
        format!(
            "{}: {}",
            typst_param_info.name,
            cast_info_to_label(&typst_param_info.input)
        )
    }

    fn param_info_to_docs(typst_param_info: &TypstParamInfo) -> Option<Documentation> {
        if !typst_param_info.docs.is_empty() {
            Some(Documentation::MarkupContent(MarkupContent {
                value: typst_param_info.docs.to_owned(),
                kind: MarkupKind::Markdown,
            }))
        } else {
            None
        }
    }

    pub fn cast_info_to_label(cast_info: &CastInfo) -> String {
        match cast_info {
            CastInfo::Any => "any".to_owned(),
            CastInfo::Value(value, _) => value.repr().to_string(),
            CastInfo::Type(ty) => ty.to_string(),
            CastInfo::Union(options) => options.iter().map(cast_info_to_label).join(" "),
        }
    }
}

#[cfg(test)]
mod test {
    use typst::syntax::Source;

    use crate::config::PositionEncoding;
    use crate::lsp_typst_boundary::lsp_to_typst;

    use super::*;

    const ENCODING_TEST_STRING: &str = "test 🥺 test";

    #[test]
    fn utf16_position_to_utf8_offset() {
        let source = Source::detached(ENCODING_TEST_STRING);

        let start = LspPosition {
            line: 0,
            character: 0,
        };
        let emoji = LspPosition {
            line: 0,
            character: 5,
        };
        let post_emoji = LspPosition {
            line: 0,
            character: 7,
        };
        let end = LspPosition {
            line: 0,
            character: 12,
        };

        let start_offset =
            lsp_to_typst::position_to_offset(start, PositionEncoding::Utf16, &source);
        let start_actual = 0;

        let emoji_offset =
            lsp_to_typst::position_to_offset(emoji, PositionEncoding::Utf16, &source);
        let emoji_actual = 5;

        let post_emoji_offset =
            lsp_to_typst::position_to_offset(post_emoji, PositionEncoding::Utf16, &source);
        let post_emoji_actual = 9;

        let end_offset = lsp_to_typst::position_to_offset(end, PositionEncoding::Utf16, &source);
        let end_actual = 14;

        assert_eq!(start_offset, start_actual);
        assert_eq!(emoji_offset, emoji_actual);
        assert_eq!(post_emoji_offset, post_emoji_actual);
        assert_eq!(end_offset, end_actual);
    }

    #[test]
    fn utf8_offset_to_utf16_position() {
        let source = Source::detached(ENCODING_TEST_STRING);

        let start = 0;
        let emoji = 5;
        let post_emoji = 9;
        let end = 14;

        let start_position = LspPosition {
            line: 0,
            character: 0,
        };
        let start_actual =
            typst_to_lsp::offset_to_position(start, PositionEncoding::Utf16, &source);

        let emoji_position = LspPosition {
            line: 0,
            character: 5,
        };
        let emoji_actual =
            typst_to_lsp::offset_to_position(emoji, PositionEncoding::Utf16, &source);

        let post_emoji_position = LspPosition {
            line: 0,
            character: 7,
        };
        let post_emoji_actual =
            typst_to_lsp::offset_to_position(post_emoji, PositionEncoding::Utf16, &source);

        let end_position = LspPosition {
            line: 0,
            character: 12,
        };
        let end_actual = typst_to_lsp::offset_to_position(end, PositionEncoding::Utf16, &source);

        assert_eq!(start_position, start_actual);
        assert_eq!(emoji_position, emoji_actual);
        assert_eq!(post_emoji_position, post_emoji_actual);
        assert_eq!(end_position, end_actual);
    }
}
