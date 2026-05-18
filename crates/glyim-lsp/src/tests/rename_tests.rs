use crate::rename::rename_symbol;
use crate::tests::test_utils::setup_test_db;
use lsp_types::{Position, RenameParams, TextDocumentIdentifier, TextDocumentPositionParams};

#[test]
fn test_rename_symbol() {
    let source = r#"fn main() {
    let x = 5;
    println!("{}", x);
}
"#;
    let (db, file_map, uri, file_id) = setup_test_db(source, "/test/main.g");
    // Find position of "x" in "let x = 5;"
    let sm = db.source_maps.read().get(&file_id).unwrap().clone();
    let offset = sm.line_col_to_offset(1, 8).unwrap(); // line 1, col 8 (after "let ")
    let (line, col) = (1, 8);
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line: line as u32, character: col as u32 },
        },
        new_name: "y".to_string(),
        work_done_progress_params: Default::default(),
    };
    let edit = rename_symbol(&db, &file_map, &params);
    assert!(edit.is_some());
    let edit = edit.unwrap();
    let changes = edit.changes.unwrap();
    let edits = changes.get(&uri).unwrap();
    // There should be two occurrences: the let binding and the println reference.
    assert_eq!(edits.len(), 2);
    for te in edits {
        assert_eq!(te.new_text, "y");
    }
}
