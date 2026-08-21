//! Tests for Tier 1.6 — scope drop elaboration (whole-value / top-level drops).
//!
//! After lowering a function body, every non-`Copy` local that is still live
//! at scope exit must receive a `Drop` terminator in reverse declaration
//! order. `Copy` locals (e.g. `i32`) must NOT be dropped.

use crate::lower::lower_body;
use crate::tests::mock_lower_ctx::TestLowerCtx;
use crate::tests::thir_builder::ThirBuilder;
use glyim_core::primitives::IntTy;
use glyim_mir::{BasicBlockIdx, LocalIdx, TerminatorKind};
use glyim_test::{assert_mir, test_ty_ctx};
use glyim_type::*;
use glyim_typeck::thir::{ExprKind, Literal};

fn string_ty(ctx: &mut TyCtxMut) -> Ty {
    ctx.mk_ty(TyKind::String)
}

#[test]
fn non_copy_local_gets_drop_terminator() {
    // `let s: String = ...;` with no other statements -> scope drop for `s`.
    let mut ctx_mut = test_ty_ctx();
    let s_ty = string_ty(&mut ctx_mut);
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();
    let mock = TestLowerCtx::new(&ctx);

    let mut b = ThirBuilder::new(s_ty, interner);
    let mut stmts = Vec::new();
    b.add_let_binding(
        "s",
        s_ty,
        Some(b.expr(ExprKind::Literal(Literal::String(b.make_name("hi"))), s_ty)),
        &mut stmts,
    );
    let body = b.into_body(stmts, vec![]);
    let result = lower_body(&mock, &body);

    // `String` is non-Copy, so scope drop elaboration must insert a `Drop`
    // terminator for the `s` local (local index 1) somewhere in the body, and
    // the entry block must not end directly in `Return` (it must route through
    // the drop chain).
    let entry = &result.body.basic_blocks[BasicBlockIdx::from_raw(0)];
    assert!(
        !matches!(entry.terminator.kind, TerminatorKind::Return),
        "entry must route through drop chain, not Return directly"
    );

    let mut drop_count = 0;
    let mut drops_s_local = false;
    for bb in result.body.basic_blocks.iter() {
        if let TerminatorKind::Drop { place, cleanup, .. } = &bb.terminator.kind {
            drop_count += 1;
            assert!(cleanup.is_none(), "drop elaboration should not set cleanup");
            if place.local == LocalIdx::from_raw(1) {
                drops_s_local = true;
            }
        }
    }
    assert!(
        drops_s_local,
        "expected a Drop terminator for the `s` local"
    );
    assert!(drop_count >= 1, "expected at least one Drop terminator");
}

#[test]
fn copy_local_does_not_get_drop_terminator() {
    // `let x: i32 = 10;` -> i32 is Copy, so no drop elaboration; the body ends
    // directly in a `Return`.
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();
    let mock = TestLowerCtx::new(&ctx);

    let mut b = ThirBuilder::new(i32_ty, interner);
    let mut stmts = Vec::new();
    b.add_let_binding(
        "x",
        i32_ty,
        Some(b.expr(ExprKind::Literal(Literal::Int(10, None)), i32_ty)),
        &mut stmts,
    );
    let body = b.into_body(stmts, vec![]);
    let result = lower_body(&mock, &body);

    // No drops: single block ending directly in Return.
    assert_eq!(result.body.basic_blocks.len(), 1);
    assert_mir(&ctx, &result.body).block_terminator(BasicBlockIdx::from_raw(0), "Return");
}

#[test]
fn return_place_is_never_dropped() {
    // A function returning a `String` value must not drop `_0` (the return
    // place) in the scope-drop elaboration.
    let mut ctx_mut = test_ty_ctx();
    let s_ty = string_ty(&mut ctx_mut);
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();
    let mock = TestLowerCtx::new(&ctx);

    let b = ThirBuilder::new(s_ty, interner);
    let body = b.into_body(vec![], vec![]);
    let result = lower_body(&mock, &body);

    // No locals other than _0, so no drop chain: single Return block.
    assert_eq!(result.body.basic_blocks.len(), 1);
    assert_mir(&ctx, &result.body).block_terminator(BasicBlockIdx::from_raw(0), "Return");
}
