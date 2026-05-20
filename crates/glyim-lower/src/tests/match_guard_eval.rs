use crate::lower::lower_body;
use crate::tests::mock_lower_ctx::TestLowerCtx;
use crate::tests::thir_builder::ThirBuilder;
use glyim_core::primitives::{BinOp, IntTy};
use glyim_test::test_ty_ctx;
use glyim_type::*;
use glyim_typeck::thir::{self, ExprKind, Literal, PatternKind};

/// S20-T03: Match guard evaluates before arm body
#[test]
fn match_guard_creates_switch_for_guard() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let bool_ty = ctx_mut.bool_ty();
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();
    let mock = TestLowerCtx::new(&ctx);

    let mut b = ThirBuilder::new(i32_ty, interner.clone());
    let mut stmts = Vec::new();
    b.add_let_binding("x", i32_ty, None, &mut stmts);

    let arm_pat = thir::Pattern {
        kind: PatternKind::Wild,
        ty: i32_ty,
        span: glyim_span::Span::DUMMY,
    };
    let guard_expr = b.expr(
        ExprKind::Binary {
            op: BinOp::Gt,
            lhs: Box::new(b.var_ref_expr("x", i32_ty)),
            rhs: Box::new(b.expr(ExprKind::Literal(Literal::Int(0, None)), i32_ty)),
        },
        bool_ty,
    );
    let arm_body = b.expr(ExprKind::Literal(Literal::Int(1, None)), i32_ty);

    let arm = thir::MatchArm {
        pat: arm_pat,
        guard: Some(Box::new(guard_expr)),
        body: arm_body,
    };

    let otherwise_pat = thir::Pattern {
        kind: PatternKind::Wild,
        ty: i32_ty,
        span: glyim_span::Span::DUMMY,
    };
    let otherwise_body = b.expr(ExprKind::Literal(Literal::Int(0, None)), i32_ty);
    let otherwise_arm = thir::MatchArm {
        pat: otherwise_pat,
        guard: None,
        body: otherwise_body,
    };

    let match_expr = b.expr(
        ExprKind::Match {
            scrutinee: Box::new(b.var_ref_expr("x", i32_ty)),
            arms: vec![arm, otherwise_arm],
        },
        i32_ty,
    );
    stmts.push(thir::Stmt::Expr { expr: match_expr });

    let body = b.into_body(stmts, vec![]);
    let result = lower_body(&mock, &body);

    // Should produce multiple blocks due to guard evaluation
    // Entry, match-switch, guard-switch, arm-body, guard-fail, otherwise-arm, merge
    assert!(
        result.body.basic_blocks.len() >= 4,
        "expected at least 4 blocks for match with guard, got {}",
        result.body.basic_blocks.len()
    );
    assert!(!result.diagnostics.iter().any(|d| d.is_error()));
}
