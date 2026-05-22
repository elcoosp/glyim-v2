use glyim_core::primitives::*;
use glyim_type::*;
use glyim_typeck::thir::*;
use crate::lower::lower_body;
use glyim_test::mock::MockLowerCtx;
use glyim_test::test_frozen_ty_ctx;

#[test]
fn or_pattern_creates_switch_int() {
    let ctx = test_frozen_ty_ctx();
    let mut mock = MockLowerCtx::new(&ctx);
    let int_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));

    // Build a match with or-pattern: match x { 0 | 1 => true, _ => false }
    let scrutinee = Expr {
        ty: int_ty,
        span: Default::default(),
        kind: ExprKind::VarRef(LocalVarId::from_raw(0)),
    };
    let arm1_body = Expr {
        ty: ctx.bool_ty(),
        span: Default::default(),
        kind: ExprKind::Literal(Literal::Bool(true)),
    };
    let arm2_body = Expr {
        ty: ctx.bool_ty(),
        span: Default::default(),
        kind: ExprKind::Literal(Literal::Bool(false)),
    };
    let or_pat = Pattern {
        ty: int_ty,
        span: Default::default(),
        kind: PatternKind::Or(vec![
            Pattern {
                ty: int_ty,
                span: Default::default(),
                kind: PatternKind::Literal(Literal::Int(0, Some(IntTy::I32))),
            },
            Pattern {
                ty: int_ty,
                span: Default::default(),
                kind: PatternKind::Literal(Literal::Int(1, Some(IntTy::I32))),
            },
        ]),
    };
    let arms = vec![
        MatchArm {
            pat: or_pat,
            guard: None,
            body: arm1_body,
        },
        MatchArm {
            pat: Pattern {
                ty: int_ty,
                span: Default::default(),
                kind: PatternKind::Wild,
            },
            guard: None,
            body: arm2_body,
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
            let keys: Vec<_> = targets.iter().map(|(v,_)| v).collect();
            if keys.contains(&0) && keys.contains(&1) {
                found_switch = true;
                break;
            }
        }
    }
    assert!(found_switch, "No SwitchInt with keys 0 and 1 found for OR pattern");
}
