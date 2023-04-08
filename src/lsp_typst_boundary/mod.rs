//! Conversions between Typst and LSP types and representations

use std::collections::HashMap;
use std::io;

use tower_lsp::lsp_types::{self, Url};

pub mod typst_invoke;
pub mod world;

pub type LspUri = lsp_types::Url;
pub type TypstPath = std::path::Path;
pub type TypstPathOwned = std::path::PathBuf;

pub type TypstSource = typst::syntax::Source;
pub type TypstSourceId = typst::syntax::SourceId;

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
    use std::path::PathBuf;

    use typst::syntax::Source;

    use super::*;

    // TODO: these URL <-> Path functions are a quick hack to make things work. They should be
    // replaced by a more comprehensive system to reliably convert `LspUri`s to `TypstPath`s
    pub fn uri_to_path(lsp_uri: &LspUri) -> TypstPathOwned {
        lsp_uri.to_file_path().unwrap_or_else(|_| PathBuf::new())
    }

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

    pub fn range(lsp_range: &LspRange, typst_source: &Source) -> TypstRange {
        let lsp_start = lsp_range.raw_range.start;
        let typst_start = position_to_offset(lsp_start, lsp_range.encoding, typst_source);

        let lsp_end = lsp_range.raw_range.end;
        let typst_end = position_to_offset(lsp_end, lsp_range.encoding, typst_source);

        TypstRange {
            start: typst_start,
            end: typst_end,
        }
    }
}

pub mod typst_to_lsp {
    use typst::syntax::Source;

    use super::*;

    // TODO: these URL <-> Path functions are a quick hack to make things work. They should be
    // replaced by a more comprehensive system to reliably convert `LspUri`s to `TypstPath`s
    pub fn path_to_uri(typst_path: &TypstPath) -> io::Result<LspUri> {
        let canonical_path = typst_path.canonicalize()?;
        let lsp_uri = Url::from_file_path(canonical_path).unwrap();
        Ok(lsp_uri)
    }

    pub fn offset_to_position(
        typst_offset: TypstOffset,
        typst_source: &Source,
        lsp_position_encoding: LspPositionEncoding,
    ) -> LspPosition {
        let line_index = typst_source.byte_to_line(typst_offset).unwrap();
        let column_index = typst_source.byte_to_column(typst_offset).unwrap();

        let lsp_line = line_index as u32;
        let lsp_column = match lsp_position_encoding {
            LspPositionEncoding::Utf8 => column_index as u32,
            LspPositionEncoding::Utf16 => {
                // See the implementation of `lsp_to_typst::position_to_offset` for discussion
                // relevent to this function.

                // TODO: Typst's `Source` could easily provide an implementation of the method we
                //   need here. Submit a PR to `typst` to add it, then update this if/when merged.

                let utf16_offset = typst_source.byte_to_utf16(typst_offset).unwrap();

                let byte_line_offset = typst_source.line_to_byte(line_index).unwrap();
                let utf16_line_offset = typst_source.byte_to_utf16(byte_line_offset).unwrap();

                let utf16_column_offset = utf16_offset - utf16_line_offset;
                utf16_column_offset as u32
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
        let lsp_start = offset_to_position(typst_start, typst_source, lsp_position_encoding);

        let typst_end = typst_range.end;
        let lsp_end = offset_to_position(typst_end, typst_source, lsp_position_encoding);

        let raw_range = LspRawRange::new(lsp_start, lsp_end);
        LspRange::new(raw_range, lsp_position_encoding)
    }
}
