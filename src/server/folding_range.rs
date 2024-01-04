use tower_lsp::lsp_types::{FoldingRange, FoldingRangeKind, Url, SymbolKind};
use typst::syntax::Source;

use super::TypstServer;

impl TypstServer {
    pub fn get_folding_ranges(&self, source: &Source, uri: &Url) -> Option<Vec<FoldingRange>> {
        let symbols = self.document_symbols(source, uri, None).collect::<Vec<_>>();

        let mut starting_line: Option<u32> = None;
        let mut ranges: Vec<FoldingRange> = Vec::new();

        for symbol in symbols.into_iter().flatten().filter(|sym| sym.kind == SymbolKind::NAMESPACE) {
            if let Some(prev_line) = starting_line {
                ranges.push(FoldingRange {
                    start_line: prev_line,
                    end_line: symbol.location.range.start.line - 1,
                    kind: Some(FoldingRangeKind::Region),
                    ..Default::default()
                })
            }

            starting_line = Some(symbol.location.range.end.line)
        }

        // we've reached the end of the document but there was still an 'open' header
        if let Some(prev_line) = starting_line {
            ranges.push(FoldingRange {
                start_line: prev_line,
                end_line: <usize as TryInto<u32>>::try_into(source.len_lines())
                    .expect("Could not convert usize into u32")
                    - 1,
                kind: Some(FoldingRangeKind::Region),
                ..Default::default()
            })
        }

        Some(ranges)
    }
}
