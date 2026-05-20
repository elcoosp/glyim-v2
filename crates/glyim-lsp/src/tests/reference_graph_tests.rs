use crate::reference_graph::ReferenceGraph;
use glyim_core::Interner;
use glyim_span::FileId;

fn build_graph_for_source(file_id: FileId, source: &str) -> (ReferenceGraph, Interner) {
    let parse_result = glyim_frontend::parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let (hir, _diags) =
        glyim_hir::pipeline_api::lower_crate_for_pipeline(&parse_result.root, &mut interner);
    let mut graph = ReferenceGraph::new();
    graph.build_from_hir(file_id, &hir, &interner);
    (graph, interner)
}

#[test]
fn s11_t01_find_references_returns_all_uses_of_function() {
    let source = "fn foo() {}\nfn bar() {\n    foo();\n    foo();\n}\n";
    let file_id = FileId::from_raw(1);
    let (graph, _interner) = build_graph_for_source(file_id, source);
    let refs = graph.find_references("foo");
    assert!(!refs.is_empty(), "Expected references to foo");

    let def_count = refs.iter().filter(|r| r.is_definition).count();
    let use_count = refs.iter().filter(|r| !r.is_definition).count();

    assert_eq!(def_count, 1, "Expected exactly one definition of foo");
    assert!(
        use_count >= 2,
        "Expected at least 2 uses of foo, got {}",
        use_count
    );
}

#[test]
fn s11_t01_find_references_no_false_positives() {
    let source = "fn main() {}";
    let file_id = FileId::from_raw(2);
    let (graph, _interner) = build_graph_for_source(file_id, source);
    let refs = graph.find_references("nonexistent");
    assert!(
        refs.is_empty(),
        "Expected no references for nonexistent symbol"
    );
}
