use crate::completion;
use crate::database::AnalysisDatabase;
use crate::symbol_index::{DefinitionLocation, SymbolInfo, SymbolKind, TypeSignature};
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use lsp_types::{
    CompletionItemKind, CompletionParams, Documentation, MarkupKind, Position,
    TextDocumentIdentifier, TextDocumentPositionParams, Url,
};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn test_provide_completions() {
    let db = Arc::new(AnalysisDatabase::new());
    let mut file_map = crate::FileMap::new();
    let path = PathBuf::from("/test/main.g");
    let file_id = file_map.get_or_create(&path);

    let mut index = db.symbol_index.write();
    index.insert_test_symbol(
        file_id,
        SymbolInfo {
            name: "my_func".to_string(),
            kind: SymbolKind::Function,
            definition: DefinitionLocation {
                file_id,
                span: Span::new(
                    FileId::from_raw(0),
                    ByteIdx::from_raw(0),
                    ByteIdx::from_raw(10),
                    SyntaxContext::ROOT,
                ),
            },
            type_signature: Some(TypeSignature {
                params: vec![("x".to_string(), "i32".to_string())],
                return_type: Some("i32".to_string()),
            }),
            is_pub: true,
            documentation: Some("Does something".to_string()),
        },
    );
    drop(index);

    let uri = Url::from_file_path(&path).unwrap();
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 0,
                character: 0,
            },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: None,
    };

    let result = completion::provide_completions(&db, &file_map, &params);

    assert!(result.is_some());
    if let lsp_types::CompletionResponse::List(list) = result.unwrap() {
        assert!(!list.items.is_empty());
        let item = &list.items[0];
        assert_eq!(item.label, "my_func");
        assert_eq!(item.kind, Some(CompletionItemKind::FUNCTION));
        assert_eq!(item.detail, Some("(x: i32) -> i32".to_string()));

        match item.documentation {
            Some(Documentation::MarkupContent(ref mc)) => {
                assert_eq!(mc.kind, MarkupKind::Markdown);
                assert_eq!(mc.value, "Does something");
            }
            _ => panic!("Expected MarkupContent documentation"),
        }
    } else {
        panic!("Expected CompletionList");
    }
}
