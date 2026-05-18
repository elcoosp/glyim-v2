use crate::{
    AnalysisDatabase, DefinitionLocation, SymbolInfo, SymbolKind, TypeSignature,
    completion::provide_completions,
};
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use lsp_types::*;
use std::path::PathBuf;
use std::sync::Arc;
use url::Url;

#[test]
fn completions_include_function_name() {
    // Create analysis database
    let analysis = AnalysisDatabase::new();

    // Create a temporary file path
    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join("main.g");

    // Manually add file mapping using FileMap's public API
    let file_id = {
        let mut file_map = analysis.file_map.write();
        file_map.get_or_create(&path)
    };

    // Add source map
    let content = "fn foo() {}\nfn main() { foo(); }";
    let source_map = crate::SourceMap::new(path.clone(), file_id, content.to_string());
    analysis.source_maps.write().insert(file_id, source_map);

    // Add symbol to index
    let span = Span::new(
        file_id,
        ByteIdx::ZERO,
        ByteIdx::from_raw(3),
        SyntaxContext::ROOT,
    );
    let foo_info = SymbolInfo {
        name: "foo".to_string(),
        kind: SymbolKind::Function,
        definition: DefinitionLocation { file_id, span },
        type_signature: Some(TypeSignature {
            params: vec![],
            return_type: Some("()".to_string()),
        }),
        is_pub: true,
        documentation: None,
    };
    {
        let mut symbol_index = analysis.symbol_index.write();
        symbol_index.insert_test_symbol(file_id, foo_info);
        // Verify insertion
        let symbols = symbol_index.symbols_in_file(file_id);
        assert!(!symbols.is_empty(), "Symbols in file should not be empty");
        assert_eq!(symbols[0].name, "foo");
    }

    // Prepare completion params
    let completions = provide_completions(
        &analysis,
        &analysis.file_map.read(),
        &CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::from_file_path(&path).unwrap(),
                },
                position: Position {
                    line: 1,
                    character: 12,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        },
    );

    assert!(completions.is_some(), "provide_completions returned None");
    if let Some(CompletionResponse::List(list)) = completions {
        let labels: Vec<_> = list.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"foo"), "Completions should include 'foo'");
    } else {
        panic!("Expected CompletionResponse::List");
    }
}
