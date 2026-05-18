use crate::AnalysisDatabase;
use lsp_types::*;

fn find_braced_ranges(source: &str) -> Vec<FoldingRange> {
    let lines: Vec<&str> = source.lines().collect();
    let mut ranges = Vec::new();
    let mut brace_stack: Vec<(usize, usize)> = Vec::new();

    for (line_idx, line) in lines.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch == '{' {
                brace_stack.push((line_idx, col));
            } else if ch == '}' && let Some((start_line, start_col)) = brace_stack.pop() {
                ranges.push(FoldingRange {
                    start_line: start_line as u32,
                    start_character: Some(start_col as u32),
                    end_line: line_idx as u32,
                    end_character: Some(col as u32),
                    kind: Some(FoldingRangeKind::Region),
                    collapsed_text: None,
                });
            }
        }
    }
    ranges
}

pub fn provide_folding_ranges(
    db: &AnalysisDatabase,
    params: &FoldingRangeParams,
) -> Option<Vec<FoldingRange>> {
    let uri = &params.text_document.uri;
    let path = uri.to_file_path().ok()?;
    let file_map = db.file_map.read();
    let file_id = file_map.get_by_path(&path)?;
    drop(file_map);
    let source_maps = db.source_maps.read();
    let sm = source_maps.get(&file_id)?;
    let source = sm.source();

    let ranges = find_braced_ranges(source);
    if ranges.is_empty() {
        None
    } else {
        Some(ranges)
    }
}
