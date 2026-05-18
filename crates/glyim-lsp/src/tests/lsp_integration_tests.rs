use crate::*;
use glyim_test::mock::TestDbBuilder;
use std::path::PathBuf;
use lsp_types::*;
use url::Url;
use glyim_span::{Span, ByteIdx, SyntaxContext};

#[test]
fn completions_include_function_name() {
    let db_builder = TestDbBuilder::new()
        .name("lsp_test")
        .target_triple("x86_64-unknown-linux-gnu")
        .opt_level(0);
    let db = db_builder.build();
    let mut lsp_state = LspState::new(db);

    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("main.g");
    let content = "fn foo() {}\nfn main() { foo(); }";
    lsp_state.did_open(path.clone(), content.to_string(), 1);

    let file_id = lsp_state.file_id(&path).unwrap();
    let span = Span::new(file_id, ByteIdx::ZERO, ByteIdx::from_raw(3), SyntaxContext::ROOT);
    let foo_info = SymbolInfo {
        name: "foo".to_string(),
        kind: crate::SymbolKind::Function,
        definition: DefinitionLocation { file_id, span },
        type_signature: Some(TypeSignature {
            params: vec![],
            return_type: Some("()".to_string()),
        }),
        is_pub: true,
        documentation: None,
    };
    {
        let mut symbol_index = lsp_state.analysis().symbol_index.write();
        symbol_index.insert_test_symbol(file_id, foo_info);
        // Verify insertion worked
        let symbols = symbol_index.symbols_in_file(file_id);
        assert!(!symbols.is_empty(), "Symbols in file should not be empty after insertion");
        assert_eq!(symbols[0].name, "foo");
    }

    let analysis = lsp_state.analysis();
    let file_map_guard = analysis.file_map.read();
    let file_map = &*file_map_guard;

    // Verify file_map contains the correct file_id
    let resolved_id = file_map.get_by_path(&path).expect("Path should be in file_map");
    assert_eq!(resolved_id, file_id);

    let completions = crate::completion::provide_completions(
        analysis,
        file_map,
        &CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::from_file_path(&path).unwrap(),
                },
                position: Position { line: 1, character: 12 },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        },
    );

    assert!(completions.is_some(), "provide_completions returned None");
    if let Some(CompletionResponse::List(list)) = completions {
        let labels: Vec<_> = list.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"foo"));
    } else {
        panic!("Expected CompletionResponse::List");
    }
}
