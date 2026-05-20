use glyim_test::test_frozen_ty_ctx;
use glyim_core::def_id::AdtId;
use glyim_lower::LowerCtx;
use crate::pipeline_context::PipelineLowerCtx;
use glyim_hir::CrateHir;

#[test]
fn adt_def_fallback_returns_empty_for_unknown_id() {
    let ty_ctx = test_frozen_ty_ctx();
    let hir = CrateHir::default(); // We need to check if CrateHir has new() or default
    let ctx = PipelineLowerCtx::new(&ty_ctx, &hir);
    let adt_id = AdtId::from_raw(999);
    let def = ctx.adt_def(adt_id);
    // Should return empty variants (fallback)
    assert_eq!(def.variants.len(), 0);
}
