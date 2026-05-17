use crate::database::AnalysisDatabase;
use crate::database::SourceMap;
use crate::navigation::goto_definition;
use crate::symbol_index::{DefinitionLocation, SymbolInfo, SymbolKind};
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
                "fn main() { let x = 42; x + 1 }".to_string(),
            ),
        );
    }
    {
        let mut idx = db.symbol_index.write();
        // 'x' identifier spans bytes 16..17 (0-based: 'l','e','t',' ','x')
        idx.insert_test_symbol(
            file_id,
            SymbolInfo {
                name: "x".into(),
                kind: SymbolKind::Local,
                definition: DefinitionLocation {
                    file_id,
                    span: make_span(file_id, 16, 17),
                },
                type_signature: None,
                is_pub: false,
                documentation: None,
            },
        );
    }
    (db, file_id, path)
}

#[test]
fn goto_definition_returns_location() {
    let (db, _file_id, path) = setup_test_db();
    let file_map = db.file_map.read();
    let uri = Url::from_file_path(&path).unwrap();
    // Position points to the first 'x' at character 16 (0-based)
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 0,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: None,
        },
    };
    let result = goto_definition(&db, &file_map, &params);
    assert!(result.is_some());
    if let Some(GotoDefinitionResponse::Scalar(loc)) = result {
        assert_eq!(loc.uri, uri);
        assert_eq!(loc.range.start.line, 0);
        assert_eq!(loc.range.start.character, 16);
        assert_eq!(loc.range.end.character, 17);
    } else {
        panic!("Expected scalar location");
    }
}
