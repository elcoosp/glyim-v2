use crate::AnalysisDatabase;
use crate::database::FileMap;
use lsp_types::*;
use std::collections::HashMap;

fn find_references_in_source(source: &str, target_name: &str) -> Vec<Range> {
    let lines: Vec<&str> = source.lines().collect();
    let mut ranges = Vec::new();
    for (line_idx, line) in lines.iter().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut j = 0;
        while j < chars.len() {
            if chars[j].is_alphabetic() || chars[j] == '_' {
                let start = j;
                while j < chars.len() && (chars[j].is_alphabetic() || chars[j] == '_') {
                    j += 1;
                }
                let word: String = chars[start..j].iter().collect();
                if word == target_name {
                    ranges.push(Range {
                        start: Position { line: line_idx as u32, character: start as u32 },
                        end: Position { line: line_idx as u32, character: j as u32 },
                    });
                }
            } else {
                j += 1;
            }
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
    while start > 0 && (chars[start-1].is_alphabetic() || chars[start-1] == '_') { start -= 1; }
    while end < chars.len() && (chars[end].is_alphabetic() || chars[end] == '_') { end += 1; }
    let old_name = &source[start..end];
    if old_name.is_empty() {
        return None;
    }

    let ranges = find_references_in_source(source, old_name);
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
