use crate::*;
use glyim_core::primitives::*;
use glyim_type::{InferVar, TyKind};

#[test]
fn test_property_generator_concrete() {
    let mut ctx = test_ty_ctx();
    let mut generator = property::arbitrary::Generator::new(42);
    let ty = generator.generate_ty(&mut ctx, 0);
    let frozen = ctx.freeze();
    assert!(!matches!(frozen.ty_kind(ty), TyKind::Error));
    property::arbitrary::sentinel_invariant(&frozen);
}

#[test]
fn test_property_generator_with_infer() {
    let mut ctx = test_ty_ctx();
    let mut infer = glyim_solve::InferenceTable::new();
    let mut generator = property::arbitrary::Generator::new(123);
    let ty = generator.generate_ty_with_infer(&mut ctx, &mut infer, 0);
    let frozen = ctx.freeze();
    let kind = frozen.ty_kind(ty);
    assert!(!matches!(kind, TyKind::Error));
}

#[test]
fn test_property_generator_depth_limit() {
    let mut ctx = test_ty_ctx();
    let mut generator = property::arbitrary::Generator::new(999).with_max_depth(0);
    let ty = generator.generate_ty(&mut ctx, 5);
    let frozen = ctx.freeze();
    assert!(!matches!(frozen.ty_kind(ty), TyKind::Error));
}

#[test]
fn test_unification_var_with_concrete() {
    let mut ctx = test_ty_ctx();
    let mut infer = glyim_solve::InferenceTable::new();
    let var = infer.new_ty_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)));
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    property::unify::test_unify_var_with_concrete(&mut ctx, &mut infer, var_ty, i32_ty);
}

#[test]
fn test_unification_same_type() {
    let mut ctx = test_ty_ctx();
    let mut infer = glyim_solve::InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    property::unify::test_unify_same_type_succeeds(&mut ctx, &mut infer, i32_ty);
}

#[test]
fn test_unification_different_types_fail() {
    let mut ctx = test_ty_ctx();
    let mut infer = glyim_solve::InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let bool_ty = ctx.bool_ty();
    property::unify::test_unify_different_types_fails(&mut ctx, &mut infer, i32_ty, bool_ty);
}

#[test]
fn test_check_ty_property() {
    let result = check_ty_property(42, 50, |_ctx, _ty| Ok(()));
    assert!(result.is_ok());
}

#[test]
fn test_check_ty_property_failure() {
    let result = check_ty_property(42, 10, |_ctx, _ty| Err("bad type".to_string()));
    assert!(result.is_err());
}
