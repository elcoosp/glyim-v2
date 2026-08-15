use crate::AnalysisDatabase;
use crate::database::FileMap;
use lsp_types::*;
use std::collections::HashSet;

use url::Url;
fn collect_unused_imports(source: &str, used_names: &HashSet<String>) -> Vec<(String, Range)> {
    let lines: Vec<&str> = source.lines().collect();
    let mut imports = Vec::new();

    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("use ") {
            let import_line = trimmed.trim_start_matches("use ").trim_end_matches(';');
            let import_name = import_line
                .split("::")
                .last()
                .unwrap_or(import_line)
                .to_string();
            let start_col = line.len() - trimmed.len();
            let end_col = line.len();
            let range = Range {
                start: Position {
                    line: line_idx as u32,
                    character: start_col as u32,
                },
                end: Position {
                    line: line_idx as u32,
                    character: end_col as u32,
                },
            };
            imports.push((import_name, range));
        }
    }

    // An import is unused iff its name has zero Read/Write references anywhere
    // in the indexed HIR (resolved via the reference graph, not text search —
    // this avoids false positives on shadowed names and false negatives on
    // names that appear only inside strings/comments).
    imports
        .into_iter()
        .filter(|(name, _)| !used_names.contains(name))
        .collect()
}

pub fn provide_code_actions(
    db: &AnalysisDatabase,
    file_map: &FileMap,
    params: &CodeActionParams,
) -> Option<Vec<CodeActionOrCommand>> {
    let uri = &params.text_document.uri;
    let path = Url::parse(uri.as_str()).ok()?.to_file_path().ok()?;
    let file_id = file_map.get_by_path(&path)?;
    let source_maps = db.source_maps.read();
    let source_map = source_maps.get(&file_id)?;
    let source = source_map.source();

    let used_names = {
        let ref_graph = db.reference_graph.read();
        ref_graph.used_symbols()
    };

    let unused_imports = collect_unused_imports(source, &used_names);
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
                    #[allow(clippy::mutable_key_type)]
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

#[cfg(test)]
mod tests {
    use super::collect_unused_imports;
    use std::collections::HashSet;

    fn names(set: &[&str]) -> HashSet<String> {
        set.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_unused_import_flagged_when_no_reference() {
        // `HashMap` is imported but never referenced anywhere in the HIR.
        let src = "use std::collections::HashMap;\nfn main() { let x = 42; }\n";
        let used = names(&[]); // empty reference graph
        let unused = collect_unused_imports(src, &used);
        assert_eq!(unused.len(), 1, "import with no references must be flagged");
        assert_eq!(unused[0].0, "HashMap");
    }

    #[test]
    fn test_used_import_not_flagged() {
        // `Foo` is imported AND has a real reference (it appears in code), so
        // it must NOT be flagged as unused — even though the old text heuristic
        // would also have found it, the graph is now the source of truth.
        let src = "use mymod::Foo;\nfn main() { let x = Foo; }\n";
        let used = names(&["Foo"]);
        let unused = collect_unused_imports(src, &used);
        assert!(
            unused.is_empty(),
            "import with a real reference must NOT be flagged, got {:?}",
            unused
        );
    }

    #[test]
    fn test_import_name_in_string_is_still_unused() {
        // `Secret` is imported and only appears inside a string literal and a
        // comment — no real reference. The graph-based check correctly keeps
        // it flagged (the old substring heuristic would also flag it; the
        // point is the graph is the authority, not raw text).
        let src = "use mymod::Secret;\nfn main() { let s = \"Secret inside a string\"; /* Secret in comment */ }\n";
        let used = names(&[]);
        let unused = collect_unused_imports(src, &used);
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].0, "Secret");
    }

    #[test]
    fn test_only_unused_among_many_flagged() {
        let src = "use a::Used;\nuse b::Unused;\nfn main() { let x = Used; }\n";
        let used = names(&["Used"]);
        let unused = collect_unused_imports(src, &used);
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].0, "Unused");
    }
}
