use glyim_core::primitives::*;
use glyim_type::*;
use glyim_typeck::thir::*;
use crate::lower::lower_body;
use glyim_test::mock::MockLowerCtx;
use glyim_test::test_frozen_ty_ctx;

#[test]
fn guard_creates_conditional_branch() {
    let ctx = test_frozen_ty_ctx();
    let mut mock = MockLowerCtx::new(&ctx);
    let int_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let opt_ty = ctx.mk_ty(TyKind::Adt(/* Option type ID */, Substitution::empty())); // simplified

    let scrutinee = Expr {
        ty: opt_ty,
        span: Default::default(),
        kind: ExprKind::VarRef(LocalVarId::from_raw(0)),
    };
    let guard_cond = Expr {
        ty: ctx.bool_ty(),
        span: Default::default(),
        kind: ExprKind::Binary {
            op: BinOp::Gt,
            lhs: Box::new(Expr {
                ty: int_ty,
                span: Default::default(),
                kind: ExprKind::VarRef(LocalVarId::from_raw(1)),
            }),
            rhs: Box::new(Expr {
                ty: int_ty,
                span: Default::default(),
                kind: ExprKind::Literal(Literal::Int(0, Some(IntTy::I32))),
            }),
        },
    };
    let some_pat = Pattern {
        ty: int_ty,
        span: Default::default(),
        kind: PatternKind::Binding {
            name: Name::from_str("y"),
            mutability: Mutability::Not,
            subpattern: None,
        },
    };
    let arms = vec![
        MatchArm {
            pat: some_pat,
            guard: Some(guard_cond),
            body: Expr {
                ty: int_ty,
                span: Default::default(),
                kind: ExprKind::VarRef(LocalVarId::from_raw(1)),
            },
        },
        MatchArm {
            pat: Pattern {
                ty: opt_ty,
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
    assert!(result.diagnostics.is_empty());

    // Check that there is at least one SwitchInt that is not the main discriminant
    let mut guard_switches = 0;
    for block in result.body.basic_blocks.iter() {
        if let glyim_mir::TerminatorKind::SwitchInt { .. } = &block.terminator.kind {
            guard_switches += 1;
        }
    }
    assert!(guard_switches >= 2, "Expected at least two SwitchInts (discriminant + guard)");
}
