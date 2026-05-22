use glyim_core::primitives::*;
use glyim_type::*;
use glyim_typeck::thir::*;
use crate::lower::lower_body;
use glyim_test::mock::MockLowerCtx;
use glyim_test::test_frozen_ty_ctx;

#[test]
fn slice_pattern_does_not_panic() {
    let ctx = test_frozen_ty_ctx();
    let mut mock = MockLowerCtx::new(&ctx);
    let int_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let arr_ty = ctx.mk_ty(TyKind::Array(int_ty, Const::from_usize(3).unwrap()));

    let scrutinee = Expr {
        ty: arr_ty,
        span: Default::default(),
        kind: ExprKind::VarRef(LocalVarId::from_raw(0)),
    };
    // Slice pattern [a, b]
    let a_pat = Pattern {
        ty: int_ty,
        span: Default::default(),
        kind: PatternKind::Binding {
            name: Name::from_str("a"),
            mutability: Mutability::Not,
            subpattern: None,
        },
    };
    let b_pat = Pattern {
        ty: int_ty,
        span: Default::default(),
        kind: PatternKind::Binding {
            name: Name::from_str("b"),
            mutability: Mutability::Not,
            subpattern: None,
        },
    };
    let slice_pat = Pattern {
        ty: arr_ty,
        span: Default::default(),
        kind: PatternKind::Slice {
            prefix: vec![a_pat, b_pat],
            slice: None,
            suffix: vec![],
        },
    };
    let arm_body = Expr {
        ty: int_ty,
        span: Default::default(),
        kind: ExprKind::Binary {
            op: BinOp::Add,
            lhs: Box::new(Expr {
                ty: int_ty,
                span: Default::default(),
                kind: ExprKind::VarRef(LocalVarId::from_raw(0)),
            }),
            rhs: Box::new(Expr {
                ty: int_ty,
                span: Default::default(),
                kind: ExprKind::VarRef(LocalVarId::from_raw(1)),
            }),
        },
    };
    let arms = vec![
        MatchArm {
            pat: slice_pat,
            guard: None,
            body: arm_body,
        },
        MatchArm {
            pat: Pattern {
                ty: arr_ty,
                span: Default::default(),
                kind: PatternKind::Wild,
            },
            guard: None,
            body: Expr {
                ty: int_ty,
                span: Default::default(),
                kind: ExprKind::Literal(Literal::Int(0, Some(IntTy::I32))),
            },
        },
    ];
    let match_expr = Expr {
        ty: int_ty,
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
        return_ty: int_ty,
        span: Default::default(),
    };

    let result = lower_body(&mock, &body);
    // We only care that it doesn't panic; diagnostics may be present due to incomplete lowering
    // but at least it should produce a body.
    assert!(result.body.basic_blocks.len() > 0, "MIR body should have blocks");
}
