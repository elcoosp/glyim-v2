//! Regression tests for Tier 1.7 — dynamic range slicing (`arr[i..j]`).
//!
//! The THIR `Range` bound can only carry a literal today (it is
//! `Option<Box<Expr>>` holding a literal expression), so a *fully*
//! runtime-`i`/`j` slice cannot yet be expressed at the THIR level. The
//! lowering still builds the slice as an ordinary Rvalue (`{ ptr, len }`
//! tuple) via `lower_dynamic_range_slice` rather than a `Place` projection,
//! with runtime arithmetic (Len / Mul / Add / Sub) and bounds-check asserts.
//! These tests lock that behavior in.

use crate::lower::lower_body;
use crate::tests::mock_lower_ctx::TestLowerCtx;
use crate::tests::thir_builder::ThirBuilder;
use glyim_core::primitives::IntTy;
use glyim_mir::{AggregateKind, Rvalue, StatementKind};
use glyim_test::test_ty_ctx;
use glyim_type::*;
use glyim_typeck::thir::{self, ExprKind, Literal};

fn array_ty(ctx: &mut TyCtxMut, elem: Ty, len: u64) -> Ty {
    let usize_ty = ctx.mk_ty(TyKind::Uint(glyim_core::primitives::UintTy::Usize));
    let len_const = glyim_type::Const {
        kind: glyim_type::ConstKind::Int(len as i128),
        ty: usize_ty,
    };
    ctx.mk_ty(TyKind::Array(elem, len_const))
}

fn int_bound(ctx: &ThirBuilder, n: i128) -> Box<thir::Expr> {
    Box::new(ctx.expr(ExprKind::Literal(Literal::Int(n, None)), ctx.return_ty))
}

/// A `base[index]` expression where `index` is a `Range` with optional bounds.
fn slice_expr(
    b: &ThirBuilder,
    base_ty: Ty,
    slice_ty: Ty,
    start: Option<i128>,
    end: Option<i128>,
    inclusive: bool,
) -> thir::Expr {
    let range = b.expr(
        ExprKind::Range {
            start: start.map(|n| int_bound(b, n)),
            end: end.map(|n| int_bound(b, n)),
            inclusive,
        },
        slice_ty,
    );
    b.expr(
        ExprKind::Index {
            base: Box::new(b.var_ref_expr("arr", base_ty)),
            index: Box::new(range),
        },
        slice_ty,
    )
}

fn lower_slice(
    start: Option<i128>,
    end: Option<i128>,
    inclusive: bool,
) -> crate::lower::LowerResult {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let arr_ty = array_ty(&mut ctx_mut, i32_ty, 5);
    let slice_ty = ctx_mut.mk_ty(TyKind::Slice(i32_ty));
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();
    let mock = TestLowerCtx::new(&ctx);

    let mut b = ThirBuilder::new(slice_ty, interner);
    let mut stmts = Vec::new();
    b.add_let_binding("arr", arr_ty, None, &mut stmts);
    stmts.push(thir::Stmt::Expr {
        expr: slice_expr(&b, arr_ty, slice_ty, start, end, inclusive),
    });

    let body = b.into_body(stmts, vec![]);
    lower_body(&mock, &body)
}

fn has_ptr_len_tuple(result: &crate::lower::LowerResult) -> bool {
    for bb in result.body.basic_blocks.iter() {
        for stmt in bb.statements.iter() {
            if let StatementKind::Assign(_, Rvalue::Aggregate(AggregateKind::Tuple, ops)) =
                &stmt.kind
                && ops.len() == 2
            {
                return true;
            }
        }
    }
    false
}

#[test]
fn range_index_lowers_to_ptr_len_tuple_without_error() {
    let result = lower_slice(Some(1), Some(4), false);
    assert!(
        result.diagnostics.is_empty(),
        "slice lowering emitted an unexpected diagnostic: {:?}",
        result.diagnostics
    );
    assert!(
        has_ptr_len_tuple(&result),
        "expected a ptr/len tuple Aggregate among the lowering statements"
    );
}

#[test]
fn inclusive_range_index_is_rejected() {
    let result = lower_slice(Some(1), Some(4), true);
    assert!(
        !result.diagnostics.is_empty(),
        "inclusive range slicing should emit a diagnostic"
    );
}

#[test]
fn open_ended_range_index_lowers() {
    let result = lower_slice(Some(1), None, false);
    assert!(
        result.diagnostics.is_empty(),
        "open-ended slice lowering emitted an unexpected diagnostic: {:?}",
        result.diagnostics
    );
    assert!(
        has_ptr_len_tuple(&result),
        "expected a ptr/len tuple Aggregate for open-ended slice"
    );
}

/// Out-of-bounds dynamic slices must hit a panic path rather than silently
/// compute a dangling `{ ptr, len }`. `lower_dynamic_range_slice` routes the
/// failing bounds-check edge to `TerminatorKind::Unreachable` (the panic
/// landing), so an `Unreachable` terminator in the lowered body is the
/// observable proof that `start <= end` / `end <= len` are asserted.
fn has_unreachable_terminator(result: &crate::lower::LowerResult) -> bool {
    result.body.basic_blocks.iter().any(|bb| {
        matches!(bb.terminator.kind, glyim_mir::TerminatorKind::Unreachable)
    })
}

#[test]
fn out_of_bounds_range_lowers_with_panic_path() {
    // `arr[2..5]` over a 3-element array: end (5) > len (3) -> the bounds
    // check must fail and route to an `Unreachable` (panic) terminator.
    let result = lower_slice(Some(2), Some(5), false);
    assert!(
        result.diagnostics.is_empty(),
        "out-of-bounds slice lowering emitted an unexpected diagnostic before bounds check: {:?}",
        result.diagnostics
    );
    assert!(
        has_unreachable_terminator(&result),
        "expected an Unreachable terminator proving the out-of-bounds panic path is emitted"
    );
    assert!(
        has_ptr_len_tuple(&result),
        "expected a ptr/len tuple Aggregate for the in-bounds computation path"
    );
}
