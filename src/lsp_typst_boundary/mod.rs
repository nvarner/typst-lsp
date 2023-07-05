//! Conversions between Typst and LSP types and representations

use std::collections::HashMap;

use tower_lsp::lsp_types;

pub mod clock;
pub mod world;

pub type LspUri = lsp_types::Url;
pub type TypstPath = std::path::Path;
pub type TypstPathOwned = std::path::PathBuf;

pub type TypstSource = typst::syntax::Source;

pub type LspPosition = lsp_types::Position;
/// The interpretation of an `LspCharacterOffset` depends on the `LspPositionEncoding`
pub type LspCharacterOffset = u32;
pub type LspPositionEncoding = crate::config::PositionEncoding;
/// Byte offset (i.e. UTF-8 bytes) in Typst files, either from the start of the line or the file
pub type TypstOffset = usize;

/// An LSP range. It needs its associated `LspPositionEncoding` to be used. The `LspRange` struct
/// provides this range with that encoding.
pub type LspRawRange = lsp_types::Range;
pub type TypstRange = std::ops::Range<usize>;

pub type LspDiagnostic = lsp_types::Diagnostic;
pub type LspDiagnostics = HashMap<LspUri, Vec<LspDiagnostic>>;
pub type TypstSourceError = typst::diag::SourceError;

pub type TypstTooltip = typst::ide::Tooltip;
pub type LspHoverContents = lsp_types::HoverContents;

pub type TypstDatetime = typst::eval::Datetime;

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
}

pub type LspCompletion = lsp_types::CompletionItem;
pub type LspCompletionKind = lsp_types::CompletionItemKind;
pub type TypstCompletion = typst::ide::Completion;
pub type TypstCompletionKind = typst::ide::CompletionKind;

pub mod lsp_to_typst {
    use anyhow::Context;
    use typst::file::FileId;

    use super::*;

    pub fn uri_to_file_id(lsp_uri: &LspUri, project_root: &TypstPath) -> anyhow::Result<FileId> {
        let path = uri_to_path(lsp_uri)?;
        let project_relative = path_to_project_relative(project_root, &path)?;
        // TODO: should we also detect URIs in packages?
        Ok(FileId::new(None, &project_relative))
    }

    fn uri_to_path(lsp_uri: &LspUri) -> anyhow::Result<TypstPathOwned> {
        lsp_uri
            .to_file_path()
            .map_err(|()| anyhow::anyhow!("could not get path for URI {lsp_uri}"))
    }

    fn path_to_project_relative<'a, 'b>(
        project_root: &TypstPath,
        path: &TypstPath,
    ) -> anyhow::Result<TypstPathOwned> {
        path.strip_prefix(project_root)
            .map(|path| TypstPath::new("/").join(path))
            .with_context(|| {
                format!(
                    "path {} is not in the project root {}",
                    path.display(),
                    project_root.display()
                )
            })
    }

    pub fn position_to_offset(
        lsp_position: LspPosition,
        lsp_position_encoding: LspPositionEncoding,
        typst_source: &TypstSource,
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

    pub fn range(lsp_range: &LspRange, source: &TypstSource) -> TypstRange {
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
    use itertools::Itertools;
    use lazy_static::lazy_static;
    use regex::{Captures, Regex};
    use tower_lsp::lsp_types::{
        DiagnosticSeverity, InsertTextFormat, LanguageString, MarkedString,
    };
    use typst::World;
    use typst_library::prelude::EcoString;

    use crate::config::ConstConfig;

    use super::world::WorkspaceWorld;
    use super::*;

    pub fn path_to_uri(typst_path: &TypstPath) -> anyhow::Result<LspUri> {
        LspUri::from_file_path(typst_path).map_err(|()| {
            let path = typst_path.to_string_lossy();
            anyhow::anyhow!("could not get URI for path {path}")
        })
    }

    pub fn offset_to_position(
        typst_offset: TypstOffset,
        lsp_position_encoding: LspPositionEncoding,
        typst_source: &TypstSource,
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
        typst_source: &TypstSource,
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
                let substitution = format!("${{{}:{}}}", counter, &cap[1]);
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

    pub fn source_error_to_diagnostic(
        typst_error: &TypstSourceError,
        world: &WorkspaceWorld,
        const_config: &ConstConfig,
    ) -> Option<(LspUri, LspDiagnostic)> {
        let typst_span = typst_error.span;
        let typst_source = world.source(typst_span.source());

        let typst_range = typst_source.range(typst_span);
        let lsp_range = range(typst_range, typst_source, const_config.position_encoding);

        let lsp_message = typst_error.message.to_string();

        let diagnostic = LspDiagnostic {
            range: lsp_range.raw_range,
            severity: Some(DiagnosticSeverity::ERROR),
            message: lsp_message,
            ..Default::default()
        };

        let uri = path_to_uri(typst_source.path()).ok()?;

        Some((uri, diagnostic))
    }

    pub fn source_errors_to_diagnostics<'a>(
        errors: impl IntoIterator<Item = &'a TypstSourceError>,
        world: &WorkspaceWorld,
        const_config: &ConstConfig,
    ) -> LspDiagnostics {
        errors
            .into_iter()
            .filter_map(|error| {
                typst_to_lsp::source_error_to_diagnostic(error, world, const_config)
            })
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
}

#[cfg(test)]
mod test {
    use crate::config::PositionEncoding;
    use crate::lsp_typst_boundary::lsp_to_typst;

    use super::*;

    const ENCODING_TEST_STRING: &str = "test 🥺 test";

    #[test]
    fn utf16_position_to_utf8_offset() {
        let source = TypstSource::detached(ENCODING_TEST_STRING);

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
        let source = TypstSource::detached(ENCODING_TEST_STRING);

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
