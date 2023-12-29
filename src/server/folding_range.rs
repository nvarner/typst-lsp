use tower_lsp::lsp_types::{FoldingRange, FoldingRangeKind};
use typst::syntax::Source;

use super::TypstServer;

impl TypstServer {
    pub fn get_folding_ranges(&self, source: &Source) -> Option<Vec<FoldingRange>> {
        let mut ranges = vec![];
        let mut previous_idx: Option<usize> = None;

        for (line_idx, line) in source.text().lines().enumerate() {
            if line.starts_with('=') {
                if let Some(prev) = previous_idx {
                    ranges.push(FoldingRange {
                        start_line: prev.try_into().unwrap(),
                        end_line: <usize as TryInto<u32>>::try_into(line_idx).unwrap() - 1u32,
                        kind: Some(FoldingRangeKind::Region),
                        ..Default::default()
                    });
                }
                previous_idx = Some(line_idx);
            }
        }

        if let Some(prev) = previous_idx {
            ranges.push(FoldingRange {
                start_line: prev.try_into().unwrap(),
                end_line: <usize as TryInto<u32>>::try_into(source.len_lines()).unwrap() - 1u32,
                kind: Some(FoldingRangeKind::Region),
                ..Default::default()
            });
        }

        Some(ranges)
    }
}
