use crate::completion::provide_completions;
use crate::database::AnalysisDatabase;
use crate::database::SourceMap;
use crate::symbol_index::{DefinitionLocation, SymbolInfo, SymbolKind, TypeSignature};
use crate::tests::helpers::make_span;
use glyim_span::FileId;
use lsp_types::*;
use std::path::PathBuf;

fn setup_test_db() -> (AnalysisDatabase, FileId, PathBuf) {
    let db = AnalysisDatabase::new();
    let file_id = FileId::from_raw(0);
    let path = PathBuf::from("/test/main.g");
    {
        let mut fm = db.file_map.write();
        fm.get_or_create(&path);
    }
    {
        let mut sm = db.source_maps.write();
        sm.insert(
            file_id,
            SourceMap::new(
                path.clone(),
                file_id,
                "struct Point { x: i32, y: i32 }\nfn main() { let p = Point { x: 10, y: 20 }; p. }"
                    .to_string(),
            ),
        );
    }
    {
        let mut idx = db.symbol_index.write();
        idx.insert_test_symbol(
            file_id,
            SymbolInfo {
                name: "x".into(),
                kind: SymbolKind::Field,
                definition: DefinitionLocation {
                    file_id,
                    span: make_span(file_id, 14, 15),
                },
                type_signature: Some(TypeSignature {
                    params: vec![],
                    return_type: Some("i32".into()),
                }),
                is_pub: true,
                documentation: None,
            },
        );
        idx.insert_test_symbol(
            file_id,
            SymbolInfo {
                name: "y".into(),
                kind: SymbolKind::Field,
                definition: DefinitionLocation {
                    file_id,
                    span: make_span(file_id, 24, 25),
                },
                type_signature: Some(TypeSignature {
                    params: vec![],
                    return_type: Some("i32".into()),
                }),
                is_pub: true,
                documentation: None,
            },
        );
    }
    (db, file_id, path)
}

#[test]
fn completion_provides_struct_fields() {
    let (db, _file_id, path) = setup_test_db();
    let file_map = db.file_map.read();
    let uri = Url::from_file_path(&path).unwrap();
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 1,
                character: 42,
            },
        },
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: None,
        },
        context: None,
    };
    let response = provide_completions(&db, &file_map, &params).expect("completion response");
    if let CompletionResponse::List(list) = response {
        let labels: Vec<&str> = list.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"x"));
        assert!(labels.contains(&"y"));
        assert_eq!(list.items.len(), 2);
    } else {
        panic!("Expected CompletionList");
    }
}
