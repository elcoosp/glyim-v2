use crate::database::{FileMap, SourceMap};
use crate::rename::rename_symbol;
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
fn test_rename_updates_all_references() {
    let content = r#"fn foo() {
    let x = foo();
}"#;
    let path = PathBuf::from("main.gly");
    let (db, file_map) = setup_analysis(content, &path);
    let uri = Url::from_file_path(&path).unwrap();
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line: 1, character: 4 }, // over 'foo'
        },
        new_name: "bar".to_string(),
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    let edit = rename_symbol(&db, &file_map, &params);
    assert!(edit.is_some());
    let edit = edit.unwrap();
    let changes = edit.changes;
    assert!(changes.is_some());
    let changes = changes.unwrap();
    let file_edits = changes.get(&uri);
    assert!(file_edits.is_some());
    // Should have two edits: one for definition (line 1) and one for reference (line 2)
    assert_eq!(file_edits.unwrap().len(), 2);
}
