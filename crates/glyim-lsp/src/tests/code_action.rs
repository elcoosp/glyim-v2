use crate::code_action::provide_code_actions;
use crate::database::{FileMap, SourceMap};
use crate::AnalysisDatabase;
use lsp_types::*;
use std::path::PathBuf;

fn setup_analysis(content: &str, path: &PathBuf) -> (AnalysisDatabase, FileMap) {
    let db = AnalysisDatabase::new();
    let mut file_map = FileMap::new();
    let file_id = file_map.get_or_create(path);
    let source_map = SourceMap::new(path.clone(), file_id, content.to_string());
    db.source_maps.write().insert(file_id, source_map);
    (db, file_map)
}

#[test]
fn test_remove_unused_import_code_action() {
    let content = r#"use std::collections::HashMap;
fn main() {
    let x = 42;
}"#;
    let path = PathBuf::from("main.gly");
    let (db, file_map) = setup_analysis(content, &path);
    let uri = Url::from_file_path(path).unwrap();
    let params = CodeActionParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        range: Range::default(),
        context: CodeActionContext::default(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let actions = provide_code_actions(&db, &file_map, &params);
    assert!(actions.is_some());
    let actions = actions.unwrap();
    assert!(!actions.is_empty());
    let action_str = format!("{:?}", actions);
    assert!(action_str.contains("Remove unused import: HashMap"));
}
