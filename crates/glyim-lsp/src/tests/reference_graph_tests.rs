use super::common::{compile_to_hir, create_test_file_id};
use crate::reference_graph::{ReferenceGraph, ReferenceKind};
use glyim_core::Interner;

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

/// Tier 6.1: `build_from_hir` must walk `Range` sides, `Closure` bodies,
/// `Index` base/operand, and `Break` values so that variables used only inside
/// those expression forms are still found by "find all references". These arms
/// previously fell through to the `_ => {}` fallback and silently skipped
/// their children.
///
/// Note on semantics: this graph records local `let`/closure bindings as
/// `Variable` *uses* (via their binding name / `Expr::Path`), not as
/// `is_definition` entries — only top-level item/function/param names become
/// definitions. So the assertions below count *uses*, which is exactly what
/// "find all references" surfaces. Each probe variable is therefore used in
/// exactly one place so we can prove its containing expression form is walked.
#[test]
fn test_reference_graph_walks_range_and_closure() {
    let src = r#"
fn main() {
    let range_only = 0;
    let closure_only = 0;
    let index_base_only = [0, 1, 2];
    let _ = index_base_only[1..range_only];
    let f = |a| closure_only + a;
}
"#;
    let mut interner = Interner::new();
    let file_id = create_test_file_id(1);
    let (hir, diags) = compile_to_hir(src, file_id, &mut interner);
    assert!(diags.is_empty(), "Compilation had diagnostics: {:?}", diags);

    let mut ref_graph = ReferenceGraph::new();
    ref_graph.build_from_hir(file_id, &hir, &interner);

    // Every probe variable below is bound with `let` (which this graph records
    // as a `Variable` use of the binding name — see the module note) and then
    // used in exactly ONE expression form. So a correct walk yields exactly 2
    // references: the `let` use + the construct use. Before the fix the
    // construct-specific arms hit `_ => {}` and silently skipped their children,
    // so the construct use was missing and only 1 reference (the `let`) showed.

    // `range_only` is used *only* as the `end` of `1..range_only` (a Range).
    let range_refs = ref_graph.find_references("range_only");
    assert_eq!(
        range_refs.len(),
        2,
        "range-only variable must be found via the Range `end` arm; got {:?}",
        range_refs
    );

    // `closure_only` is used *only* inside the closure body `|a| closure_only + a`.
    let closure_refs = ref_graph.find_references("closure_only");
    assert_eq!(
        closure_refs.len(),
        2,
        "closure-only variable must be found via the closure body; got {:?}",
        closure_refs
    );

    // `index_base_only` is used as the `base` of `index_base_only[..]` (an Index).
    let index_refs = ref_graph.find_references("index_base_only");
    assert_eq!(
        index_refs.len(),
        2,
        "index base must add the Index use to the let use; got {:?}",
        index_refs
    );
}
