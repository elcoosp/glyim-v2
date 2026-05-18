use crate::hover::provide_hover;
use crate::{AnalysisDatabase, DefinitionLocation, SymbolInfo, SymbolKind, TypeSignature};
use glyim_span::{ByteIdx, Span, SyntaxContext};
use lsp_types::*;
use std::path::PathBuf;
use url::Url;

fn get_test_path(filename: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(filename);
    path
}

#[test]
#[ignore]
fn hover_shows_type_signature_and_doc() {
    let analysis = AnalysisDatabase::new();
    let path = get_test_path("test.g");
    let file_id = {
        let mut file_map = analysis.file_map.write();
        file_map.get_or_create(&path)
    };
    let span = Span::new(
        file_id,
        ByteIdx::ZERO,
        ByteIdx::from_raw(10),
        SyntaxContext::ROOT,
    );
    let source_map = crate::SourceMap::new(
        path.clone(),
        file_id,
        "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
    );
    analysis.source_maps.write().insert(file_id, source_map);

    let sym = SymbolInfo {
        name: "add".to_string(),
        kind: SymbolKind::Function,
        definition: DefinitionLocation {
            file_id,
            span: span.clone(),
        },
        type_signature: Some(TypeSignature {
            params: vec![
                ("a".to_string(), "i32".to_string()),
                ("b".to_string(), "i32".to_string()),
            ],
            return_type: Some("i32".to_string()),
        }),
        is_pub: true,
        documentation: Some("Adds two numbers".to_string()),
    };
    analysis
        .symbol_index
        .write()
        .insert_test_symbol(file_id, sym);

    let file_map_guard = analysis.file_map.read();
    let file_map = &*file_map_guard;

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: Url::from_file_path(&path).unwrap(),
            },
            position: Position {
                line: 0,
                character: 4,
            },
        },
        work_done_progress_params: Default::default(),
    };
    let hover = provide_hover(&analysis, file_map, &params);
    assert!(hover.is_some());
    let content = &hover.unwrap().contents;
    if let HoverContents::Markup(markup) = content {
        assert!(markup.value.contains("fn add(a: i32, b: i32) -> i32"));
        assert!(markup.value.contains("Adds two numbers"));
    } else {
        panic!("Expected Markup content");
    }
}
