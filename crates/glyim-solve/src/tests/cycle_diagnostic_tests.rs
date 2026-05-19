use crate::*;
use glyim_test::test_ty_ctx;
use glyim_type::*;

#[test]
fn test_infinite_type_cycle_diagnostic() {
    // Part 1: Occurs check prevents creating infinite types via unify
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_ty_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)));
    let vec_ty = ctx.mk_ty(TyKind::Slice(var_ty));
    let result = infer.unify(&mut ctx, var_ty, vec_ty, glyim_span::Span::DUMMY);
    assert!(result.is_err(), "Occurs check should reject infinite type");

    // Part 2: Self‑referential variable detected by resolve_ty_shallow
    let mut ctx2 = test_ty_ctx();
    let mut infer2 = InferenceTable::new();
    let var2 = infer2.new_ty_var(&mut ctx2);
    let var_ty2 = ctx2.mk_ty(TyKind::Infer(InferVar::Ty(var2)));
    // Manually create a self‑loop: ?T = ?T
    infer2.set_ty_var_value(var2, var_ty2);
    let resolved = infer2.resolve_ty_shallow(&ctx2, var_ty2);
    assert_eq!(
        resolved,
        Ty::ERROR,
        "Self‑referential variable should resolve to ERROR"
    );
    let diags = infer2.take_diagnostics();
    assert!(
        !diags.is_empty(),
        "Should have emitted diagnostic for cycle"
    );
    assert!(diags[0].message.contains("infinite type cycle"));
}
