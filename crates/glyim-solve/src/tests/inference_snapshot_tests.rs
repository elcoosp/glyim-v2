use crate::*;
use glyim_test::test_ty_ctx;
use glyim_type::*;

#[test]
fn test_snapshot_rollback() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();

    let var1 = infer.new_ty_var(&mut ctx);
    let _ty1 = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var1)));

    let snapshot = infer.snapshot();

    let var2 = infer.new_ty_var(&mut ctx);
    let ty2 = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var2)));

    let unit = ctx.unit_ty();
    infer
        .unify(&mut ctx, ty2, unit, glyim_span::Span::DUMMY)
        .unwrap();

    assert!(infer.probe_ty_var(var2).is_some());

    infer.rollback_to(snapshot);

    assert!(infer.probe_ty_var(var1).is_none());
}
