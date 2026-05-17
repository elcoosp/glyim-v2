use crate::AnalysisDatabase;
use crate::code_action::provide_code_actions;
use crate::database::SourceMap;
use lsp_types::*;

fn setup_analysis(content: &str) -> (AnalysisDatabase, Url) {
    let db = AnalysisDatabase::new();
    let path = std::env::current_dir().unwrap().join("main.gly");
    let uri = Url::from_file_path(&path).unwrap();
    let file_id = db.file_map.write().get_or_create(&path);
    let source_map = SourceMap::new(path, file_id, content.to_string());
    db.source_maps.write().insert(file_id, source_map);
    (db, uri)
}

#[test]
fn test_remove_unused_import_code_action() {
    let content = r#"use std::collections::HashMap;
fn main() {
    let x = 42;
}"#;
    let (db, uri) = setup_analysis(content);
    let params = CodeActionParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        range: Range::default(),
        context: CodeActionContext::default(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let file_map_guard = db.file_map.read();
    let actions = provide_code_actions(&db, &*file_map_guard, &params);
    drop(file_map_guard);
    assert!(actions.is_some());
    let actions = actions.unwrap();
    assert!(!actions.is_empty());
    let action_str = format!("{:?}", actions);
    assert!(action_str.contains("Remove unused import: HashMap"));
}
