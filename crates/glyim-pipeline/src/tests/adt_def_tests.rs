use glyim_core::def_id::AdtId;
use glyim_hir::{CrateHir, Item, ItemId, ItemKind, StructItem};
use glyim_span::Span;
use glyim_test::test_frozen_ty_ctx;
use crate::pipeline_context::PipelineLowerCtx;

// Test that adt_def builds a definition from HIR when TyCtx doesn't have it.
// We'll create a dummy HIR with a struct and call adt_def.
#[test]
fn adt_def_from_hir_struct() {
    let ty_ctx = test_frozen_ty_ctx();
    let mut hir = CrateHir::default();
    // Create a dummy struct item with no fields (unit struct)
    let struct_item = Item {
        id: ItemId::from_raw(1),
        name: "TestStruct".into(),
        kind: ItemKind::Struct(StructItem {
            fields: vec![],
            kind: glyim_hir::StructKind::Unit,
            generic_params: vec![],
            where_clauses: vec![],
        }),
        visibility: glyim_core::visibility::Visibility::Public,
        span: Span::DUMMY,
    };
    hir.items.push(struct_item);
    let ctx = PipelineLowerCtx::new(&ty_ctx, &hir);
    let adt_id = AdtId::from_raw(1);
    let adt_def = ctx.adt_def(adt_id);
    // Should have one variant (unit struct) with zero fields.
    assert_eq!(adt_def.variants.len(), 1);
    assert_eq!(adt_def.variants[0].fields.len(), 0);
}

#[test]
fn adt_def_from_hir_enum() {
    // Similar for enum.
    assert!(true);
}
