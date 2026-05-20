use crate::AnalysisDatabase;
use crate::database::FileMap;
use crate::navigation::find_references;
use crate::reference_graph::{Reference, ReferenceGraph, ReferenceKind};
use glyim_span::{ByteIdx, Span, SyntaxContext};
use lsp_types::{Position, Url};
use std::path::PathBuf;
use std::sync::Arc;

fn setup_test_db_with_references() -> (Arc<AnalysisDatabase>, FileMap, PathBuf) {
    let db = Arc::new(AnalysisDatabase::new());
    let mut file_map = FileMap::new();
    let path = PathBuf::from("/test/main.gly");
    let file_id = file_map.get_or_create(&path);

    // Build a reference graph manually for testing
    let mut graph = ReferenceGraph::new();
    let span = Span::new(
        file_id,
        ByteIdx::from_raw(0),
        ByteIdx::from_raw(5),
        SyntaxContext::ROOT,
    );
    let def_ref = Reference {
        file_id,
        span,
        is_definition: true,
        kind: ReferenceKind::Definition,
    };
    let use_ref = Reference {
        file_id,
        span,
        is_definition: false,
        kind: ReferenceKind::Call,
    };
    graph.insert_test_reference("foo", def_ref);
    graph.insert_test_reference("foo", use_ref);

    // Create a source map for the file so that get_symbol_name_at_position works
    let source_map =
        crate::database::SourceMap::new(path.clone(), file_id, "fn foo() { foo(); }".to_string());
    db.source_maps.write().insert(file_id, source_map);
    *db.reference_graph.write() = graph;

    (db, file_map, path)
}

#[test]
fn find_references_returns_locations() {
    let (db, file_map, path) = setup_test_db_with_references();
    let params = lsp_types::ReferenceParams {
        text_document_position: lsp_types::TextDocumentPositionParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: Url::from_file_path(&path).unwrap(),
            },
            position: Position {
                line: 0,
                character: 3,
            }, // Position on "foo"
        },
        work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
        context: lsp_types::ReferenceContext {
            include_declaration: true,
        },
    };
    let result = find_references(&db, &file_map, &params);
    assert!(result.is_some());
    let locations = result.unwrap();
    assert!(!locations.is_empty());
}
