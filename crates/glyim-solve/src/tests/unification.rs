use glyim_core::primitives::{FloatTy, IntTy, Mutability};
use crate::*;
use glyim_test::*;
use glyim_type::*;

#[test]
fn test_unify_i32_i32_ok() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let result = infer.unify(&mut ctx, i32_ty, i32_ty, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
}

#[test]
fn test_unify_i32_u32_err() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let u32_ty = ctx.mk_ty(TyKind::Uint(glyim_core::primitives::UintTy::U32));
    let result = infer.unify(&mut ctx, i32_ty, u32_ty, glyim_span::Span::DUMMY);
    assert!(result.is_err());
}

#[test]
fn test_ty_var_binds_to_i32() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_ty_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)));
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    infer.unify(&mut ctx, var_ty, i32_ty, glyim_span::Span::DUMMY).unwrap();
    let bound = infer.probe_ty_var(var);
    assert_eq!(bound, Some(i32_ty));
}

#[test]
fn test_int_var_binds_to_i32() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_int_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Int(var)));
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    infer.unify(&mut ctx, var_ty, i32_ty, glyim_span::Span::DUMMY).unwrap();
    let bound = infer.probe_int_var(var);
    assert_eq!(bound, Some(i32_ty));
}

#[test]
fn test_int_var_bool_err() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_int_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Int(var)));
    let bool_ty = ctx.bool_ty();
    let result = infer.unify(&mut ctx, var_ty, bool_ty, glyim_span::Span::DUMMY);
    assert!(result.is_err());
}

#[test]
fn test_int_var_f64_err() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_int_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Int(var)));
    let f64_ty = ctx.mk_ty(TyKind::Float(FloatTy::F64));
    let result = infer.unify(&mut ctx, var_ty, f64_ty, glyim_span::Span::DUMMY);
    assert!(result.is_err());
}

#[test]
fn test_float_var_binds_to_f64() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_float_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Float(var)));
    let f64_ty = ctx.mk_ty(TyKind::Float(FloatTy::F64));
    infer.unify(&mut ctx, var_ty, f64_ty, glyim_span::Span::DUMMY).unwrap();
    let bound = infer.probe_float_var(var);
    assert_eq!(bound, Some(f64_ty));
}

#[test]
fn test_float_var_i32_err() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_float_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Float(var)));
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let result = infer.unify(&mut ctx, var_ty, i32_ty, glyim_span::Span::DUMMY);
    assert!(result.is_err());
}

#[test]
fn test_ref_mut_mismatch_err() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let inner = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let ref_mut = ctx.mk_ref(Region::Erased, inner, Mutability::Mut);
    let ref_shared = ctx.mk_ref(Region::Erased, inner, Mutability::Not);
    let result = infer.unify(&mut ctx, ref_mut, ref_shared, glyim_span::Span::DUMMY);
    assert!(result.is_err());
}

#[test]
fn test_ref_unify_recursive() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var_a = infer.new_ty_var(&mut ctx);
    let var_b = infer.new_ty_var(&mut ctx);
    let ty_a = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var_a)));
    let ty_b = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var_b)));
    let ref_a = ctx.mk_ref(Region::Erased, ty_a, Mutability::Not);
    let ref_b = ctx.mk_ref(Region::Erased, ty_b, Mutability::Not);
    infer.unify(&mut ctx, ref_a, ref_b, glyim_span::Span::DUMMY).unwrap();
    let resolved = infer.resolve_ty_shallow(&ctx, ty_a);
    let resolved2 = infer.resolve_ty_shallow(&ctx, ty_b);
    assert_eq!(resolved, resolved2);
}

#[test]
fn test_error_unifies_with_anything() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let result = infer.unify(&mut ctx, Ty::ERROR, i32_ty, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
}

#[test]
fn test_resolve_ty_shallow_single_binding() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_ty_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)));
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    infer.unify(&mut ctx, var_ty, i32_ty, glyim_span::Span::DUMMY).unwrap();
    let resolved = infer.resolve_ty_shallow(&ctx, var_ty);
    assert_eq!(resolved, i32_ty);
}

#[test]
fn test_resolve_ty_shallow_transitive() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var1 = infer.new_ty_var(&mut ctx);
    let var2 = infer.new_ty_var(&mut ctx);
    let ty1 = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var1)));
    let ty2 = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var2)));
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    infer.unify(&mut ctx, ty2, i32_ty, glyim_span::Span::DUMMY).unwrap();
    infer.unify(&mut ctx, ty1, ty2, glyim_span::Span::DUMMY).unwrap();
    let resolved = infer.resolve_ty_shallow(&ctx, ty1);
    assert_eq!(resolved, i32_ty);
}

#[test]
fn test_fully_resolve_unresolved_ty_var_err() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_ty_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)));
    let result = infer.fully_resolve(&ctx, var_ty);
    assert!(result.is_err());
    let err_vars = result.unwrap_err();
    assert_eq!(err_vars, vec![var]);
}

#[test]
fn test_fully_resolve_unresolved_int_var_err() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_int_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Int(var)));
    let result = infer.fully_resolve(&ctx, var_ty);
    assert!(result.is_err());
}

#[test]
fn test_fully_resolve_unresolved_float_var_err() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_float_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Float(var)));
    let result = infer.fully_resolve(&ctx, var_ty);
    assert!(result.is_err());
}

#[test]
fn test_fully_resolve_ok_for_i32() {
    let mut ctx = test_ty_ctx();
    let infer = InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let result = infer.fully_resolve(&ctx, i32_ty);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), i32_ty);
}

#[test]
fn test_distinct_indices() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let tv = infer.new_ty_var(&mut ctx);
    let iv = infer.new_int_var(&mut ctx);
    let fv = infer.new_float_var(&mut ctx);
    assert_ne!(
        std::mem::discriminant(&InferVar::Ty(tv)),
        std::mem::discriminant(&InferVar::Int(iv))
    );
    assert_ne!(
        std::mem::discriminant(&InferVar::Ty(tv)),
        std::mem::discriminant(&InferVar::Float(fv))
    );
}
