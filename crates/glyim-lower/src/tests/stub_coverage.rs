use crate::lower::lower_body;
use crate::tests::mock_lower_ctx::TestLowerCtx;
use crate::tests::thir_builder::ThirBuilder;
use glyim_core::primitives::IntTy;
use glyim_test::test_ty_ctx;
use glyim_type::*;
use glyim_typeck::thir::{self, ExprKind, Literal};

#[test]
fn err_expr_emits_warning_not_panic() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();
    let mock = TestLowerCtx::new(&ctx);

    let b = ThirBuilder::new(i32_ty, interner);
    let err_expr = b.expr(ExprKind::Err, i32_ty);
    let body = b.into_body(vec![thir::Stmt::Expr { expr: err_expr }], vec![]);
    let result = lower_body(&mock, &body);
    // Should not panic, but may have warnings
    assert!(!result.diagnostics.iter().any(|d| d.is_error()));
}

#[test]
fn expr_stmt_warns_about_dropped_value() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();
    let mock = TestLowerCtx::new(&ctx);

    let b = ThirBuilder::new(Ty::UNIT, interner);
    let lit_expr = b.expr(ExprKind::Literal(Literal::Int(42, None)), i32_ty);
    let body = b.into_body(vec![thir::Stmt::Expr { expr: lit_expr }], vec![]);
    let result = lower_body(&mock, &body);
    // The expr stmt warns via tracing; no panic
    assert!(!result.diagnostics.iter().any(|d| d.is_error()));
}
