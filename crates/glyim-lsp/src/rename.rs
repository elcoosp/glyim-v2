use crate::AnalysisDatabase;
use crate::database::FileMap;
use lsp_types::*;
use std::collections::HashMap;

fn find_all_occurrences(source: &str, target: &str) -> Vec<Range> {
    let lines: Vec<&str> = source.lines().collect();
    let mut ranges = Vec::new();
    for (line_idx, line) in lines.iter().enumerate() {
        let mut search_start = 0;
        while let Some(pos) = line[search_start..].find(target) {
            let absolute_pos = search_start + pos;
            let end_pos = absolute_pos + target.len();
            ranges.push(Range {
                start: Position {
                    line: line_idx as u32,
                    character: absolute_pos as u32,
                },
                end: Position {
                    line: line_idx as u32,
                    character: end_pos as u32,
                },
            });
            search_start = absolute_pos + 1;
        }
    }
    ranges
}

pub fn rename_symbol(
    db: &AnalysisDatabase,
    file_map: &FileMap,
    params: &RenameParams,
) -> Option<WorkspaceEdit> {
    let uri = &params.text_document_position.text_document.uri;
    let path = uri.to_file_path().ok()?;
    let file_id = file_map.get_by_path(&path)?;
    let source_maps = db.source_maps.read();
    let sm = source_maps.get(&file_id)?;
    let source = sm.source();

    let pos = params.text_document_position.position;
    let offset = sm.line_col_to_offset(pos.line as usize, pos.character as usize)?;
    let chars: Vec<char> = source.chars().collect();
    let mut start = offset;
    let mut end = offset;
    while start > 0 && (chars[start - 1].is_alphabetic() || chars[start - 1] == '_') {
        start -= 1;
    }
    while end < chars.len() && (chars[end].is_alphabetic() || chars[end] == '_') {
        end += 1;
    }
    let old_name = &source[start..end];
    if old_name.is_empty() {
        return None;
    }

    let ranges = find_all_occurrences(source, old_name);
    if ranges.is_empty() {
        return None;
    }

    let mut changes = HashMap::new();
    let mut edits = Vec::new();
    for range in ranges {
        edits.push(TextEdit {
            range,
            new_text: params.new_name.clone(),
        });
    }
    changes.insert(uri.clone(), edits);

    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}
