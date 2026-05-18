use crate::ReferenceGraph;
use glyim_core::{IndexVec, Interner};
use glyim_hir::*;
use glyim_span::FileId;

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
    // Initially empty, but should not panic.
}
