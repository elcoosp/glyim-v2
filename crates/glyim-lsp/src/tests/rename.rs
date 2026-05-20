use crate::AnalysisDatabase;
use crate::database::SourceMap;
use crate::rename::rename_symbol;
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
fn test_rename_updates_all_references() {
    let content = r#"fn foo() {
    let x = foo();
}"#;
    let (db, uri) = setup_analysis(content);
    // Position the cursor over the 'f' of 'foo' in the definition (line 0, column 3)
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 0,
                character: 3,
            },
        },
        new_name: "bar".to_string(),
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    let file_map_guard = db.file_map.read();
    let edit = rename_symbol(&db, &file_map_guard, &params);
    drop(file_map_guard);
    assert!(edit.is_some());
    let edit = edit.unwrap();
    let changes = edit.changes;
    assert!(changes.is_some());
    let changes = changes.unwrap();
    let file_edits = changes.get(&uri);
    assert!(file_edits.is_some());
    // Should have two edits: definition (line 0) and reference (line 1)
    assert_eq!(file_edits.unwrap().len(), 2);
}
