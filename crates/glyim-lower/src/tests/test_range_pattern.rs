use glyim_core::primitives::*;
use glyim_type::*;
use glyim_typeck::thir::*;
use crate::lower::lower_body;
use glyim_test::mock::MockLowerCtx;
use glyim_test::test_frozen_ty_ctx;

#[test]
fn range_pattern_creates_switch_int() {
    let ctx = test_frozen_ty_ctx();
    let mut mock = MockLowerCtx::new(&ctx);
    let int_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));

    let scrutinee = Expr {
        ty: int_ty,
        span: Default::default(),
        kind: ExprKind::VarRef(LocalVarId::from_raw(0)),
    };
    let arm_body = Expr {
        ty: ctx.bool_ty(),
        span: Default::default(),
        kind: ExprKind::Literal(Literal::Bool(true)),
    };
    let range_pat = Pattern {
        ty: int_ty,
        span: Default::default(),
        kind: PatternKind::Range {
            start: Some(Literal::Int(0, Some(IntTy::I32))),
            end: Some(Literal::Int(9, Some(IntTy::I32))),
            inclusive: true,
        },
    };
    let arms = vec![
        MatchArm {
            pat: range_pat,
            guard: None,
            body: arm_body,
        },
        MatchArm {
            pat: Pattern {
                ty: int_ty,
                span: Default::default(),
                kind: PatternKind::Wild,
            },
            guard: None,
            body: Expr {
                ty: ctx.bool_ty(),
                span: Default::default(),
                kind: ExprKind::Literal(Literal::Bool(false)),
            },
        },
    ];
    let match_expr = Expr {
        ty: ctx.bool_ty(),
        span: Default::default(),
        kind: ExprKind::Match {
            scrutinee: Box::new(scrutinee),
            arms,
        },
    };
    let body = Body {
        owner: Default::default(),
        params: vec![],
        stmts: vec![],
        expr: match_expr,
        return_ty: ctx.bool_ty(),
        span: Default::default(),
    };

    let result = lower_body(&mock, &body);
    assert!(result.diagnostics.is_empty());

    let mut found_switch = false;
    for block in result.body.basic_blocks.iter() {
        if let glyim_mir::TerminatorKind::SwitchInt { targets, .. } = &block.terminator.kind {
            if targets.iter().count() >= 10 {
                found_switch = true;
                break;
            }
        }
    }
    assert!(found_switch, "No SwitchInt with at least 10 targets found for 0..=9");
}
