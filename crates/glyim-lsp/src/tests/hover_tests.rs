use crate::database::AnalysisDatabase;
use crate::hover::provide_hover;
use crate::symbol_index::{SymbolInfo, SymbolKind, DefinitionLocation, TypeSignature};
use crate::tests::helpers::make_span;
use glyim_span::{FileId};
use crate::database::SourceMap;
use std::path::PathBuf;
use lsp_types::*;

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
        sm.insert(file_id, SourceMap::new(path.clone(), file_id, "fn add(a: i32, b: i32) -> i32 { a + b }".to_string()));
    }
    {
        let mut idx = db.symbol_index.write();
        idx.insert_test_symbol(file_id, SymbolInfo {
            name: "add".into(),
            kind: SymbolKind::Function,
            definition: DefinitionLocation { file_id, span: make_span(file_id, 0, 3) },
            type_signature: Some(TypeSignature {
                params: vec![("a".into(), "i32".into()), ("b".into(), "i32".into())],
                return_type: Some("i32".into()),
            }),
            is_pub: false,
            documentation: Some("Adds two integers".into()),
        });
    }
    (db, file_id, path)
}

#[test]
fn hover_shows_type_signature_and_doc() {
    let (db, _file_id, path) = setup_test_db();
    let file_map = db.file_map.read();
    let uri = Url::from_file_path(&path).unwrap();
    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position { line: 0, character: 0 },
        },
        work_done_progress_params: WorkDoneProgressParams { work_done_token: None },
    };
    let hover = provide_hover(&db, &file_map, &params).expect("hover should be Some");
    if let HoverContents::Markup(markup) = hover.contents {
        assert!(markup.value.contains("add"));
        assert!(markup.value.contains("a: i32"));
        assert!(markup.value.contains("b: i32"));
        assert!(markup.value.contains("-> i32"));
        assert!(markup.value.contains("Adds two integers"));
    } else {
        panic!("Expected Markup content");
    }
}
