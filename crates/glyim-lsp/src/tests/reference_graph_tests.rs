use crate::{Reference, ReferenceGraph, ReferenceKind};
use glyim_core::{IndexVec, Interner};
use glyim_hir::*;
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};

#[test]
fn build_from_hir_populates_references() {
    let mut graph = ReferenceGraph::new();
    let file_id = FileId::from_raw(1);
    let hir = CrateHir {
        items: IndexVec::new(),
        bodies: IndexVec::new(),
        body_owners: IndexVec::new(),
    };
    let interner = Interner::default();
    graph.build_from_hir(file_id, &hir, &interner);
}

#[test]
fn build_references_from_hir() {
    let mut graph = ReferenceGraph::new();
    let file_id = FileId::from_raw(100);
    let interner = Interner::default();
    let hir = CrateHir {
        items: IndexVec::new(),
        bodies: IndexVec::new(),
        body_owners: IndexVec::new(),
    };
    graph.build_from_hir(file_id, &hir, &interner);

    let span = Span::new(
        file_id,
        ByteIdx::ZERO,
        ByteIdx::from_raw(10),
        SyntaxContext::ROOT,
    );
    let reference = Reference {
        file_id,
        span,
        is_definition: false,
        kind: ReferenceKind::Call,
    };
    graph.insert_test_reference("some_func", reference.clone());

    let found = graph.find_references("some_func");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, ReferenceKind::Call);
}
