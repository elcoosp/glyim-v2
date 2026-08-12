use crate::code_action::provide_code_actions;
use crate::tests::test_utils::setup_test_db;
use lsp_types::{CodeActionParams, TextDocumentIdentifier};

#[test]
fn test_code_action_removes_unused_import() {
    let source = r#"use std::collections::HashMap;

fn main() {
    let x = 5;
}
"#;
    let (db, file_map, uri, _file_id) = setup_test_db(source, "/test/main.g");
    let params = CodeActionParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        range: Default::default(),
        context: Default::default(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let actions = provide_code_actions(&db, &file_map, &params);
    assert!(actions.is_some());
    let actions = actions.unwrap();
    assert!(!actions.is_empty());
    let action = &actions[0];
    if let lsp_types::CodeActionOrCommand::CodeAction(ca) = action {
        assert!(ca.title.contains("Remove unused import"));
        assert_eq!(ca.kind, Some(lsp_types::CodeActionKind::QUICKFIX));
        let edit = ca.edit.as_ref().unwrap();
        let changes = edit.changes.as_ref().unwrap();
        let edits = changes.get(&uri).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "");
    } else {
        panic!("Expected CodeAction");
    }
}
