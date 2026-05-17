use crate::AnalysisDatabase;
use crate::database::FileMap;
use lsp_types::*;
use std::collections::HashSet;

fn collect_unused_imports(source: &str) -> Vec<(String, Range)> {
    let lines: Vec<&str> = source.lines().collect();
    let mut imports = Vec::new();
    let mut used_names = HashSet::new();

    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("use ") {
            let import_line = trimmed.trim_start_matches("use ").trim_end_matches(';');
            let import_name = import_line.split("::").last().unwrap_or(import_line).to_string();
            let start_col = line.len() - trimmed.len();
            let end_col = line.len();
            let range = Range {
                start: Position { line: line_idx as u32, character: start_col as u32 },
                end: Position { line: line_idx as u32, character: end_col as u32 },
            };
            imports.push((import_name, range));
        }
    }

    for line in lines.iter() {
        if line.trim_start().starts_with("use ") {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace()
            .flat_map(|w| w.split(|c: char| !c.is_alphabetic() && c != '_'))
            .filter(|w| !w.is_empty())
            .collect();
        for word in words {
            used_names.insert(word.to_string());
        }
    }

    imports.into_iter()
        .filter(|(name, _)| !used_names.contains(name))
        .collect()
}

pub fn provide_code_actions(
    db: &AnalysisDatabase,
    file_map: &FileMap,
    params: &CodeActionParams,
) -> Option<Vec<CodeActionOrCommand>> {
    let uri = &params.text_document.uri;
    let path = uri.to_file_path().ok()?;
    let file_id = file_map.get_by_path(&path)?;
    let source_maps = db.source_maps.read();
    let source_map = source_maps.get(&file_id)?;
    let source = source_map.source();

    let unused_imports = collect_unused_imports(source);
    if unused_imports.is_empty() {
        return None;
    }

    let mut actions = Vec::new();
    for (import_name, range) in unused_imports {
        let edit = TextEdit {
            range,
            new_text: String::new(),
        };
        let action = CodeAction {
            title: format!("Remove unused import: {}", import_name),
            kind: Some(CodeActionKind::QUICKFIX),
            edit: Some(WorkspaceEdit {
                changes: Some({
                    let mut map = std::collections::HashMap::new();
                    map.insert(uri.clone(), vec![edit]);
                    map
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        actions.push(CodeActionOrCommand::CodeAction(action));
    }

    Some(actions)
}
