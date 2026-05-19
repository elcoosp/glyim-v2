use crate::*;
use glyim_core::primitives::{IntTy, UintTy};
use glyim_test::test_ty_ctx;
use glyim_type::*;

#[test]
fn test_coerce_array_to_slice() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let usize_ty = ctx_mut.mk_ty(TyKind::Uint(UintTy::Usize));
    let len_const = Const {
        kind: ConstKind::Int(3),
        ty: usize_ty,
    };
    let array_ty = ctx_mut.mk_ty(TyKind::Array(i32_ty, len_const));
    let slice_ty = ctx_mut.mk_ty(TyKind::Slice(i32_ty));
    let ctx = ctx_mut.freeze();
    assert!(crate::fulfill::can_coerce(&ctx, array_ty, slice_ty));
    assert!(!crate::fulfill::can_coerce(&ctx, slice_ty, array_ty));
}

#[test]
fn test_coerce_identity() {
    let mut ctx_mut = test_ty_ctx();
    let bool_ty = ctx_mut.bool_ty();
    let ctx = ctx_mut.freeze();
    assert!(crate::fulfill::can_coerce(&ctx, bool_ty, bool_ty));
}

#[test]
fn test_coerce_ref_subtyping() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let ref_i32 = ctx_mut.mk_ref(
        Region::Erased,
        i32_ty,
        glyim_core::primitives::Mutability::Not,
    );
    let ref_i32_again = ctx_mut.mk_ref(
        Region::Erased,
        i32_ty,
        glyim_core::primitives::Mutability::Not,
    );
    let ctx = ctx_mut.freeze();
    assert!(crate::fulfill::can_coerce(&ctx, ref_i32, ref_i32_again));
}

#[test]
fn test_solver_coerce_proven() {
    use crate::solver::{SimpleTraitSolver, SolverResult, TraitContext};
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let usize_ty = ctx_mut.mk_ty(TyKind::Uint(UintTy::Usize));
    let len_const = Const {
        kind: ConstKind::Int(3),
        ty: usize_ty,
    };
    let array_ty = ctx_mut.mk_ty(TyKind::Array(i32_ty, len_const));
    let slice_ty = ctx_mut.mk_ty(TyKind::Slice(i32_ty));
    let ctx = ctx_mut.freeze();
    let trait_ctx = TraitContext::new();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let coerce_pred = Predicate::Coerce(array_ty, slice_ty);
    let result = solver.evaluate_predicate(&ctx, &coerce_pred);
    assert_eq!(result, SolverResult::Proven);
}
