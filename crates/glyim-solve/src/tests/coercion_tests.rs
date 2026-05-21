//! Tests for type coercions: array→slice, &mut→&, &→*const, &mut→*mut, *mut→*const

use crate::{SimpleTraitSolver, SolverResult, TraitContext};
use crate::solver::TraitSolver;
use glyim_core::primitives::{IntTy, Mutability, UintTy};
use glyim_test::with_fresh_ty_ctx;
use glyim_type::{Const, ConstKind, Predicate, Region, TyKind};

fn mk_int_ty(ctx: &mut glyim_type::TyCtxMut, ty: IntTy) -> glyim_type::Ty {
    ctx.mk_ty(TyKind::Int(ty))
}

fn mk_uint_ty(ctx: &mut glyim_type::TyCtxMut, ty: UintTy) -> glyim_type::Ty {
    ctx.mk_ty(TyKind::Uint(ty))
}

fn mk_array(ctx: &mut glyim_type::TyCtxMut, elem: glyim_type::Ty, len: i128) -> glyim_type::Ty {
    let len_const = Const {
        kind: ConstKind::Int(len),
        ty: mk_uint_ty(ctx, UintTy::Usize),
    };
    ctx.mk_ty(TyKind::Array(elem, len_const))
}

fn mk_slice(ctx: &mut glyim_type::TyCtxMut, elem: glyim_type::Ty) -> glyim_type::Ty {
    ctx.mk_ty(TyKind::Slice(elem))
}

fn mk_ref(
    ctx: &mut glyim_type::TyCtxMut,
    inner: glyim_type::Ty,
    mutability: Mutability,
) -> glyim_type::Ty {
    ctx.mk_ref(Region::Erased, inner, mutability)
}

fn mk_raw_ptr(
    ctx: &mut glyim_type::TyCtxMut,
    inner: glyim_type::Ty,
    mutability: Mutability,
) -> glyim_type::Ty {
    ctx.mk_ty(TyKind::RawPtr(inner, mutability))
}

#[test]
fn coerce_array_to_slice() {
    let (ctx, (a, b)) = with_fresh_ty_ctx(|ctx_mut| {
        let elem = mk_int_ty(ctx_mut, IntTy::I32);
        let arr = mk_array(ctx_mut, elem, 3);
        let slice = mk_slice(ctx_mut, elem);
        (arr, slice)
    });
    let trait_ctx = TraitContext::new();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let predicate = Predicate::Coerce(a, b);
    let result = solver.evaluate_predicate(&ctx, &predicate);
    assert_eq!(result, SolverResult::Proven);
}

#[test]
fn coerce_mut_ref_to_imm_ref() {
    let (ctx, (a, b)) = with_fresh_ty_ctx(|ctx_mut| {
        let inner = mk_int_ty(ctx_mut, IntTy::I32);
        let mut_ref = mk_ref(ctx_mut, inner, Mutability::Mut);
        let imm_ref = mk_ref(ctx_mut, inner, Mutability::Not);
        (mut_ref, imm_ref)
    });
    let trait_ctx = TraitContext::new();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let predicate = Predicate::Coerce(a, b);
    let result = solver.evaluate_predicate(&ctx, &predicate);
    assert_eq!(result, SolverResult::Proven);
}

#[test]
fn coerce_imm_ref_to_const_raw_ptr() {
    let (ctx, (a, b)) = with_fresh_ty_ctx(|ctx_mut| {
        let inner = mk_int_ty(ctx_mut, IntTy::I32);
        let imm_ref = mk_ref(ctx_mut, inner, Mutability::Not);
        let raw_ptr = mk_raw_ptr(ctx_mut, inner, Mutability::Not);
        (imm_ref, raw_ptr)
    });
    let trait_ctx = TraitContext::new();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let predicate = Predicate::Coerce(a, b);
    let result = solver.evaluate_predicate(&ctx, &predicate);
    assert_eq!(result, SolverResult::Proven);
}

#[test]
fn coerce_mut_ref_to_mut_raw_ptr() {
    let (ctx, (a, b)) = with_fresh_ty_ctx(|ctx_mut| {
        let inner = mk_int_ty(ctx_mut, IntTy::I32);
        let mut_ref = mk_ref(ctx_mut, inner, Mutability::Mut);
        let raw_ptr = mk_raw_ptr(ctx_mut, inner, Mutability::Mut);
        (mut_ref, raw_ptr)
    });
    let trait_ctx = TraitContext::new();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let predicate = Predicate::Coerce(a, b);
    let result = solver.evaluate_predicate(&ctx, &predicate);
    assert_eq!(result, SolverResult::Proven);
}

#[test]
fn coerce_mut_raw_ptr_to_const_raw_ptr() {
    let (ctx, (a, b)) = with_fresh_ty_ctx(|ctx_mut| {
        let inner = mk_int_ty(ctx_mut, IntTy::I32);
        let mut_ptr = mk_raw_ptr(ctx_mut, inner, Mutability::Mut);
        let const_ptr = mk_raw_ptr(ctx_mut, inner, Mutability::Not);
        (mut_ptr, const_ptr)
    });
    let trait_ctx = TraitContext::new();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let predicate = Predicate::Coerce(a, b);
    let result = solver.evaluate_predicate(&ctx, &predicate);
    assert_eq!(result, SolverResult::Proven);
}

#[test]
fn coerce_invalid_imm_ref_to_mut_raw_ptr_fails() {
    let (ctx, (a, b)) = with_fresh_ty_ctx(|ctx_mut| {
        let inner = mk_int_ty(ctx_mut, IntTy::I32);
        let imm_ref = mk_ref(ctx_mut, inner, Mutability::Not);
        let mut_raw = mk_raw_ptr(ctx_mut, inner, Mutability::Mut);
        (imm_ref, mut_raw)
    });
    let trait_ctx = TraitContext::new();
    let mut solver = SimpleTraitSolver::new(&trait_ctx);
    let predicate = Predicate::Coerce(a, b);
    let result = solver.evaluate_predicate(&ctx, &predicate);
    assert_eq!(result, SolverResult::Ambiguous);
}
