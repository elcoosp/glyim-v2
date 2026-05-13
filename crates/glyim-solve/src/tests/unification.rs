use crate::*;
use glyim_core::primitives::{FloatTy, IntTy, Mutability};
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
    infer
        .unify(&mut ctx, var_ty, i32_ty, glyim_span::Span::DUMMY)
        .unwrap();
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
    infer
        .unify(&mut ctx, var_ty, i32_ty, glyim_span::Span::DUMMY)
        .unwrap();
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
    infer
        .unify(&mut ctx, var_ty, f64_ty, glyim_span::Span::DUMMY)
        .unwrap();
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
    infer
        .unify(&mut ctx, ref_a, ref_b, glyim_span::Span::DUMMY)
        .unwrap();
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
    infer
        .unify(&mut ctx, var_ty, i32_ty, glyim_span::Span::DUMMY)
        .unwrap();
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
    infer
        .unify(&mut ctx, ty2, i32_ty, glyim_span::Span::DUMMY)
        .unwrap();
    infer
        .unify(&mut ctx, ty1, ty2, glyim_span::Span::DUMMY)
        .unwrap();
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

#[test]
fn test_ty_var_binds_to_ty_var() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var1 = infer.new_ty_var(&mut ctx);
    let var2 = infer.new_ty_var(&mut ctx);
    let ty1 = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var1)));
    let ty2 = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var2)));
    infer.unify(&mut ctx, ty1, ty2, glyim_span::Span::DUMMY).unwrap();
    // One variable should be bound to the other
    let bound1 = infer.probe_ty_var(var1);
    let bound2 = infer.probe_ty_var(var2);
    assert!(bound1.is_some() || bound2.is_some());
}

#[test]
fn test_int_var_binds_to_int_var() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var1 = infer.new_int_var(&mut ctx);
    let var2 = infer.new_int_var(&mut ctx);
    let ty1 = ctx.mk_ty(TyKind::Infer(InferVar::Int(var1)));
    let ty2 = ctx.mk_ty(TyKind::Infer(InferVar::Int(var2)));
    infer.unify(&mut ctx, ty1, ty2, glyim_span::Span::DUMMY).unwrap();
    // One variable should be bound to the other (or both)
    let bound1 = infer.probe_int_var(var1);
    let bound2 = infer.probe_int_var(var2);
    assert!(bound1.is_some() || bound2.is_some());
}

#[test]
fn test_float_var_binds_to_float_var() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var1 = infer.new_float_var(&mut ctx);
    let var2 = infer.new_float_var(&mut ctx);
    let ty1 = ctx.mk_ty(TyKind::Infer(InferVar::Float(var1)));
    let ty2 = ctx.mk_ty(TyKind::Infer(InferVar::Float(var2)));
    infer.unify(&mut ctx, ty1, ty2, glyim_span::Span::DUMMY).unwrap();
    let bound1 = infer.probe_float_var(var1);
    let bound2 = infer.probe_float_var(var2);
    assert!(bound1.is_some() || bound2.is_some());
}

#[test]
fn test_general_ty_var_accepts_int_var() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let general = infer.new_ty_var(&mut ctx);
    let int_var = infer.new_int_var(&mut ctx);
    let general_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(general)));
    let int_ty = ctx.mk_ty(TyKind::Infer(InferVar::Int(int_var)));
    // General TyVar should accept anything, so this unify should succeed.
    infer.unify(&mut ctx, general_ty, int_ty, glyim_span::Span::DUMMY).unwrap();
    // The general var should now be bound to the int type.
    assert!(infer.probe_ty_var(general).is_some());
}

#[test]
fn test_int_var_accepts_general_ty_var() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let int_var = infer.new_int_var(&mut ctx);
    let general = infer.new_ty_var(&mut ctx);
    let int_ty = ctx.mk_ty(TyKind::Infer(InferVar::Int(int_var)));
    let general_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(general)));
    // IntVar unified with a general TyVar binds the general var to the int var.
    infer.unify(&mut ctx, int_ty, general_ty, glyim_span::Span::DUMMY).unwrap();
    assert!(infer.probe_ty_var(general).is_some());
    // The general var should be bound to the int type (the second argument).
    assert_eq!(infer.resolve_ty_shallow(&ctx, general_ty), int_ty);
}

#[test]
fn test_float_var_accepts_general_ty_var() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let float_var = infer.new_float_var(&mut ctx);
    let general = infer.new_ty_var(&mut ctx);
    let float_ty = ctx.mk_ty(TyKind::Infer(InferVar::Float(float_var)));
    let general_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(general)));
    // FloatVar unified with a general TyVar binds the general var to the float var.
    infer.unify(&mut ctx, float_ty, general_ty, glyim_span::Span::DUMMY).unwrap();
    assert!(infer.probe_ty_var(general).is_some());
    assert_eq!(infer.resolve_ty_shallow(&ctx, general_ty), float_ty);
}

#[test]
fn test_unify_ref_binds_inner_variable() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let inner_var = infer.new_ty_var(&mut ctx);
    let inner_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(inner_var)));
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let ref_with_var = ctx.mk_ref(Region::Erased, inner_ty, Mutability::Not);
    let ref_with_i32 = ctx.mk_ref(Region::Erased, i32_ty, Mutability::Not);
    infer.unify(&mut ctx, ref_with_var, ref_with_i32, glyim_span::Span::DUMMY).unwrap();
    // The inner variable should now be bound to i32
    let bound = infer.probe_ty_var(inner_var);
    assert_eq!(bound, Some(i32_ty));
}

#[test]
fn test_fully_resolve_bound_ty_var_ok() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_ty_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)));
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    infer.unify(&mut ctx, var_ty, i32_ty, glyim_span::Span::DUMMY).unwrap();
    let result = infer.fully_resolve(&ctx, var_ty);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), i32_ty);
}

#[test]
fn test_error_does_not_bind_variable() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_ty_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)));
    infer.unify(&mut ctx, Ty::ERROR, var_ty, glyim_span::Span::DUMMY).unwrap();
    assert!(infer.probe_ty_var(var).is_none());
}

#[test]
fn test_unify_refs_produces_region_constraint() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let r1 = Region::Erased.clone();
    let r2 = Region::Erased.clone();
    let ref1 = ctx.mk_ref(r1.clone(), i32_ty, Mutability::Not);
    let ref2 = ctx.mk_ref(r2.clone(), i32_ty, Mutability::Not);
    let result = infer.unify(&mut ctx, ref1, ref2, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
    let constraints = result.unwrap();
    assert_eq!(constraints.len(), 1);
    match &constraints[0] {
        Constraint::RegionEq { a, b } => {
            assert_eq!(a, &r1);
            assert_eq!(b, &r2);
        }
        _ => panic!("expected RegionEq"),
    }
}

#[test]
fn test_universe_creation() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    assert_eq!(infer.universe().0, 0);
    let u1 = infer.create_universe();
    assert_eq!(u1.0, 1);
    assert_eq!(infer.universe().0, 1);
}

#[test]
fn test_inference_table_default() {
    let infer = InferenceTable::default();
    assert_eq!(infer.universe(), UniverseIndex(0));
}
#[test]
fn test_resolve_ty_shallow_no_binding() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_ty_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)));
    let resolved = infer.resolve_ty_shallow(&ctx, var_ty);
    assert_eq!(resolved, var_ty);
}

#[test]
fn test_unify_tuple_equal_length() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let _u32_ty = ctx.mk_ty(TyKind::Uint(glyim_core::primitives::UintTy::U32));
    let bool_ty = ctx.bool_ty();
    let subst = ctx.intern_substitution(vec![
        GenericArg::Ty(i32_ty),
        GenericArg::Ty(bool_ty),
    ]);
    let tup_a = ctx.mk_ty(TyKind::Tuple(subst));
    let tup_b = ctx.mk_ty(TyKind::Tuple(subst));
    let result = infer.unify(&mut ctx, tup_a, tup_b, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
}

#[test]
fn test_unify_array_equal() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let const_val = glyim_type::Const {
        kind: glyim_type::ConstKind::Int(10),
        ty: i32_ty,
    };
    let arr_a = ctx.mk_ty(TyKind::Array(i32_ty, const_val.clone()));
    let arr_b = ctx.mk_ty(TyKind::Array(i32_ty, const_val));
    let result = infer.unify(&mut ctx, arr_a, arr_b, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
}

#[test]
fn test_unify_slice_equal() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let slice_a = ctx.mk_ty(TyKind::Slice(i32_ty));
    let slice_b = ctx.mk_ty(TyKind::Slice(i32_ty));
    let result = infer.unify(&mut ctx, slice_a, slice_b, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
}

#[test]
fn test_unify_raw_ptr_equal() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let ptr_a = ctx.mk_ty(TyKind::RawPtr(i32_ty, Mutability::Mut));
    let ptr_b = ctx.mk_ty(TyKind::RawPtr(i32_ty, Mutability::Mut));
    let result = infer.unify(&mut ctx, ptr_a, ptr_b, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
}

#[test]
fn test_unify_fn_ptr_equal() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let inputs = ctx.intern_substitution(vec![GenericArg::Ty(i32_ty)]);
    let sig = FnSig {
        inputs,
        output: i32_ty,
        c_variadic: false,
        unsafety: glyim_core::primitives::Safety::Safe,
        abi: glyim_core::primitives::Abi::Glyim,
    };
    let fn_a = ctx.mk_ty(TyKind::FnPtr(sig.clone()));
    let fn_b = ctx.mk_ty(TyKind::FnPtr(sig));
    let result = infer.unify(&mut ctx, fn_a, fn_b, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
}

#[test]
fn test_fully_resolve_nested_ref() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let inner_var = infer.new_ty_var(&mut ctx);
    let inner_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(inner_var)));
    let ref_ty = ctx.mk_ref(Region::Erased, inner_ty, Mutability::Not);
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    infer.unify(&mut ctx, inner_ty, i32_ty, glyim_span::Span::DUMMY).unwrap();
    let frozen = ctx.freeze();
    let result = infer.fully_resolve(&frozen, ref_ty);
    assert!(result.is_ok());
    // fully_resolve guarantees all vars are bound, but may not have substituted them.
    // Resolve the inner type manually.
    if let TyKind::Ref(_, inner, _) = frozen.ty_kind(ref_ty) {
        let concrete_inner = infer.resolve_ty_shallow(&frozen, *inner);
        assert_ty(&frozen, concrete_inner).is_int(IntTy::I32);
    } else {
        panic!("expected ref type");
    }
}

#[test]
fn test_fully_resolve_deeply_nested() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_ty_var(&mut ctx);
    let var_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)));
    let inner_ref = ctx.mk_ref(Region::Erased, var_ty, Mutability::Not);
    let outer_ref = ctx.mk_ref(Region::Erased, inner_ref, Mutability::Mut);
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    infer.unify(&mut ctx, var_ty, i32_ty, glyim_span::Span::DUMMY).unwrap();
    let frozen = ctx.freeze();
    let result = infer.fully_resolve(&frozen, outer_ref);
    assert!(result.is_ok());
    // fully_resolve guarantees all vars are bound, but may not have substituted them.
    // Resolve inner chain manually.
    if let TyKind::Ref(_, inner, _) = frozen.ty_kind(outer_ref) {
        if let TyKind::Ref(_, deep_inner, _) = frozen.ty_kind(*inner) {
            let concrete_inner = infer.resolve_ty_shallow(&frozen, *deep_inner);
            assert_ty(&frozen, concrete_inner).is_int(IntTy::I32);
        } else {
            panic!("expected inner ref");
        }
    } else {
        panic!("expected outer ref");
    }
}

#[test]
fn test_unify_never_with_anything_ok() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let result = infer.unify(&mut ctx, Ty::NEVER, i32_ty, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
}

#[test]
fn test_unify_unit_with_unit_ok() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let result = infer.unify(&mut ctx, Ty::UNIT, Ty::UNIT, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
}

#[test]
fn test_unify_bool_with_bool_ok() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let result = infer.unify(&mut ctx, Ty::BOOL, Ty::BOOL, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
}

#[test]
fn test_unify_char_with_char_ok() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let char_ty_a = ctx.mk_ty(TyKind::Char);
    let char_ty_b = ctx.mk_ty(TyKind::Char);
    let result = infer.unify(&mut ctx, char_ty_a, char_ty_b, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
}

#[test]
fn test_unify_string_with_string_ok() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let str_a = ctx.mk_ty(TyKind::String);
    let str_b = ctx.mk_ty(TyKind::String);
    let result = infer.unify(&mut ctx, str_a, str_b, glyim_span::Span::DUMMY);
    assert!(result.is_ok());
}

#[test]
fn test_unify_different_ints_error() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let i64_ty = ctx.mk_ty(TyKind::Int(IntTy::I64));
    let result = infer.unify(&mut ctx, i32_ty, i64_ty, glyim_span::Span::DUMMY);
    assert!(result.is_err());
}

#[test]
fn test_unify_string_with_slice_u8() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let str_ty = ctx.mk_ty(TyKind::String);
    let u8_ty = ctx.mk_ty(TyKind::Uint(glyim_core::primitives::UintTy::U8));
    let u8_slice = ctx.mk_ty(TyKind::Slice(u8_ty));
    let result = infer.unify(&mut ctx, str_ty, u8_slice, glyim_span::Span::DUMMY);
    assert!(result.is_err());
}

#[test]
fn test_probe_unbound_returns_none() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let var = infer.new_ty_var(&mut ctx);
    assert_eq!(infer.probe_ty_var(var), None);
}

#[test]
fn test_constrained_general_var_becomes_integer() {
    let mut ctx = test_ty_ctx();
    let mut infer = InferenceTable::new();
    let gen_var = infer.new_ty_var(&mut ctx);
    let gen_ty = ctx.mk_ty(TyKind::Infer(InferVar::Ty(gen_var)));
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    infer.unify(&mut ctx, gen_ty, i32_ty, glyim_span::Span::DUMMY).unwrap();
    // After binding, resolve should give i32
    let resolved = infer.resolve_ty_shallow(&ctx, gen_ty);
    assert_eq!(resolved, i32_ty);
}
