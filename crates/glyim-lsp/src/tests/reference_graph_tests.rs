use super::common::{compile_to_hir, create_test_file_id};
use crate::reference_graph::{ReferenceGraph, ReferenceKind};
use crate::symbol_index::{DefinitionLocation, SymbolIndex, SymbolInfo, SymbolKind};
use glyim_core::Interner;
use glyim_span::{FileId, Span};

#[test]
fn test_find_references_on_function_finds_call_sites() {
    let src = r#"
fn foo() {}
fn bar() {
    foo();
    foo();
}
"#;
    let mut interner = Interner::new();
    let file_id = create_test_file_id(1);
    let (hir, diags) = compile_to_hir(src, file_id, &mut interner);
    assert!(diags.is_empty(), "Compilation had diagnostics: {:?}", diags);

    let mut ref_graph = ReferenceGraph::new();
    ref_graph.build_from_hir(file_id, &hir, &interner);

    let symbol_name = "foo";
    let refs = ref_graph.find_references(symbol_name);
    // Should have one definition and two calls
    assert_eq!(
        refs.len(),
        3,
        "Expected 3 references (1 def + 2 calls), got {}",
        refs.len()
    );

    let mut def_count = 0;
    let mut call_count = 0;
    for r in refs {
        if r.is_definition {
            def_count += 1;
            assert_eq!(r.kind, ReferenceKind::Definition);
        } else {
            assert_eq!(r.kind, ReferenceKind::Call);
            call_count += 1;
        }
    }
    assert_eq!(def_count, 1);
    assert_eq!(call_count, 2);
}

#[test]
fn test_reference_graph_cross_file_use() {
    let mut interner = Interner::new();
    let file_id_a = create_test_file_id(1);
    let file_id_b = create_test_file_id(2);

    let src_a = r#"
fn helper() {}
"#;
    let (hir_a, diags_a) = compile_to_hir(src_a, file_id_a, &mut interner);
    assert!(diags_a.is_empty());

    let src_b = r#"
fn caller() {
    helper();
}
"#;
    let (hir_b, diags_b) = compile_to_hir(src_b, file_id_b, &mut interner);
    assert!(diags_b.is_empty());

    let mut ref_graph = ReferenceGraph::new();
    ref_graph.build_from_hir(file_id_a, &hir_a, &interner);
    ref_graph.build_from_hir(file_id_b, &hir_b, &interner);

    let refs = ref_graph.find_references("helper");
    // Definition in file A, call in file B
    assert_eq!(refs.len(), 2);
    let def = refs.iter().find(|r| r.is_definition).unwrap();
    assert_eq!(def.file_id, file_id_a);
    let call = refs.iter().find(|r| !r.is_definition).unwrap();
    assert_eq!(call.file_id, file_id_b);
    assert_eq!(call.kind, ReferenceKind::Call);
}
