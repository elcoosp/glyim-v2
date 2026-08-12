use crate::AnalysisDatabase;
use crate::database::FileMap;
use lsp_types::{Position, Range, RenameParams, TextEdit, Uri, WorkspaceEdit};
use std::collections::HashMap;
use std::str::FromStr;
use url::Url;

pub fn rename_symbol(
    db: &AnalysisDatabase,
    file_map: &FileMap,
    params: &RenameParams,
) -> Option<WorkspaceEdit> {
    let uri = &params.text_document_position.text_document.uri;
    let path = Url::parse(uri.as_str()).ok()?.to_file_path().ok()?;
    let file_id = file_map.get_by_path(&path)?;
    let source_maps = db.source_maps.read();
    let sm = source_maps.get(&file_id)?;
    let pos = params.text_document_position.position;
    let offset = sm.line_col_to_offset(pos.line as usize, pos.character as usize)?;
    let source = sm.source();

    // Find symbol name at cursor
    let chars: Vec<char> = source.chars().collect();
    let mut start = offset;
    let mut end = offset;
    while start > 0 && (chars[start - 1].is_alphabetic() || chars[start - 1] == '_') {
        start -= 1;
    }
    while end < chars.len() && (chars[end].is_alphabetic() || chars[end] == '_') {
        end += 1;
    }
    if start == end {
        return None;
    }
    let symbol_name = &source[start..end];

    // First, try using the reference graph.
    let ref_graph = db.reference_graph.read();
    let references = ref_graph.find_references(symbol_name);
    if !references.is_empty() {
        // We have semantic references, use them.
        let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
        for r in references {
            if let Some(ref_path) = file_map.path(r.file_id) {
                if let Ok(ref_url) = Url::from_file_path(ref_path) {
                    let ref_uri = Uri::from_str(&ref_url.to_string()).ok()?;
                    if let Some(sm_ref) = source_maps.get(&r.file_id) {
                        if let Some(((start_line, start_col), (end_line, end_col))) =
                            sm_ref.span_to_position(r.span.lo.to_usize(), r.span.hi.to_usize())
                        {
                            let range = Range {
                                start: Position {
                                    line: start_line as u32,
                                    character: start_col as u32,
                                },
                                end: Position {
                                    line: end_line as u32,
                                    character: end_col as u32,
                                },
                            };
                            let edit = TextEdit {
                                range,
                                new_text: params.new_name.clone(),
                            };
                            changes.entry(ref_uri).or_insert_with(Vec::new).push(edit);
                        }
                    }
                }
            }
        }
        if changes.is_empty() {
            return None;
        }
        return Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        });
    }

    // Fallback: simple text-based search within the current file only.
    eprintln!("rename: fallback to text search for '{}'", symbol_name);
    let lines: Vec<&str> = source.lines().collect();
    let mut edits = Vec::new();
    for (line_idx, line) in lines.iter().enumerate() {
        let mut search_start = 0;
        while let Some(pos) = line[search_start..].find(symbol_name) {
            let abs_pos = search_start + pos;
            let end_pos = abs_pos + symbol_name.len();
            let prev = if abs_pos > 0 {
                line.chars().nth(abs_pos - 1).unwrap_or(' ')
            } else {
                ' '
            };
            let next = if end_pos < line.len() {
                line.chars().nth(end_pos).unwrap_or(' ')
            } else {
                ' '
            };
            if (prev.is_alphabetic() || prev == '_') || (next.is_alphabetic() || next == '_') {
                search_start = abs_pos + 1;
                continue;
            }
            edits.push(TextEdit {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: abs_pos as u32,
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: end_pos as u32,
                    },
                },
                new_text: params.new_name.clone(),
            });
            search_start = abs_pos + 1;
        }
    }
    if edits.is_empty() {
        return None;
    }
    #[allow(clippy::mutable_key_type)]
    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);
    Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    })
}
