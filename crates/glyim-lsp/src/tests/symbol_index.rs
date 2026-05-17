use crate::symbol_index::{SymbolIndex, SymbolInfo, SymbolKind, DefinitionLocation, TypeSignature};
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};

#[test]
fn test_insert_and_lookup() {
    let mut index = SymbolIndex::new();
    let file_id = FileId::from_raw(1);

    let sym = SymbolInfo {
        name: "foo".to_string(),
        kind: SymbolKind::Function,
        definition: DefinitionLocation {
            file_id,
            span: Span::new(file_id, ByteIdx::from_raw(0), ByteIdx::from_raw(3), SyntaxContext::ROOT),
        },
        type_signature: Some(TypeSignature {
            params: vec![],
            return_type: Some("i32".to_string()),
        }),
        is_pub: true,
        documentation: None,
    };

    index.insert_test_symbol(file_id, sym.clone());

    // Lookup by name
    let results = index.lookup_by_name("foo");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "foo");

    // Lookup by location
    let found = index.lookup_by_location(file_id, 0);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "foo");

    // Symbols in file
    let file_syms = index.symbols_in_file(file_id);
    assert_eq!(file_syms.len(), 1);

    // Query
    let query_res = index.query("f", 10);
    assert!(!query_res.is_empty());
    assert_eq!(query_res[0].name, "foo");
}

#[test]
fn test_clear_file() {
    let mut index = SymbolIndex::new();
    let file_id = FileId::from_raw(1);

    index.insert_test_symbol(
        file_id,
        SymbolInfo {
            name: "bar".to_string(),
            kind: SymbolKind::Struct,
            definition: DefinitionLocation {
                file_id,
                span: Span::new(file_id, ByteIdx::from_raw(0), ByteIdx::from_raw(3), SyntaxContext::ROOT),
            },
            type_signature: None,
            is_pub: true,
            documentation: None,
        },
    );

    assert_eq!(index.symbols_in_file(file_id).len(), 1);

    index.clear_file(file_id);

    assert_eq!(index.symbols_in_file(file_id).len(), 0);
    assert!(index.lookup_by_name("bar").is_empty());
}
