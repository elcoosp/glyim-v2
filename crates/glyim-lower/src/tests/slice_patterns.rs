use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::primitives::{IntTy, Mutability};
use glyim_mir::{BasicBlockIdx, LocalIdx, Operand, Rvalue, StatementKind};
use glyim_span::Span;
use glyim_type::{Ty, TyCtxMut, TyKind};
use glyim_typeck::thir::{self, Body, Expr, ExprKind, Literal, MatchArm, Pattern, PatternKind, Stmt};
use crate::tests::support::MockLowerCtx;

fn make_slice_scrutinee(ctx_mut: &mut TyCtxMut) -> (Ty, Expr) {
    let elem_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let slice_ty = ctx_mut.mk_ty(TyKind::Slice(elem_ty));
    let var_expr = Expr {
        kind: ExprKind::VarRef(thir::LocalVarId::from_raw(0)),
        ty: slice_ty,
        span: Span::DUMMY,
    };
    (slice_ty, var_expr)
}

#[test]
fn slice_pattern_generates_len_switch() {
    let mut ctx_mut = glyim_test::test_ty_ctx();
    let (slice_ty, scrutinee) = make_slice_scrutinee(&mut ctx_mut);
    let elem_ty = match ctx_mut.ty_kind(slice_ty) { TyKind::Slice(ety) => *ety, _ => unreachable!() };
    let intern = ctx_mut.resolver();

    // Pattern: [a, b, .., c]
    let prefix = vec![
        Pattern {
            kind: PatternKind::Binding { name: intern.intern("a"), mutability: Mutability::Not, subpattern: None },
            ty: elem_ty,
            span: Span::DUMMY,
        },
        Pattern {
            kind: PatternKind::Binding { name: intern.intern("b"), mutability: Mutability::Not, subpattern: None },
            ty: elem_ty,
            span: Span::DUMMY,
        },
    ];
    let suffix = vec![
        Pattern {
            kind: PatternKind::Binding { name: intern.intern("c"), mutability: Mutability::Not, subpattern: None },
            ty: elem_ty,
            span: Span::DUMMY,
        },
    ];
    let pat = Pattern {
        kind: PatternKind::Slice { prefix, slice: None, suffix },
        ty: slice_ty,
        span: Span::DUMMY,
    };

    let arm_body = Expr {
        kind: ExprKind::Literal(Literal::Unit),
        ty: ctx_mut.unit_ty(),
        span: Span::DUMMY,
    };
    let arm = MatchArm { pat, guard: None, body: arm_body };
    let match_expr = Expr {
        kind: ExprKind::Match { scrutinee: Box::new(scrutinee), arms: vec![arm] },
        ty: ctx_mut.unit_ty(),
        span: Span::DUMMY,
    };
    let stmt = Stmt::Expr { expr: match_expr };
    let body = Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        params: vec![],
        return_ty: ctx_mut.unit_ty(),
        stmts: vec![stmt],
        span: Span::DUMMY,
    };

    let ctx = ctx_mut.freeze();
    let lower_ctx = MockLowerCtx::new(&ctx);
    let result = crate::lower_body(&lower_ctx, &body);
    assert!(result.diagnostics.is_empty(), "Lowering failed: {:?}", result.diagnostics);

    let mir_body = result.body;
    let entry_block = &mir_body.basic_blocks[BasicBlockIdx::from_raw(0)];
    let has_len = entry_block.statements.iter().any(|s| matches!(s.kind, StatementKind::Assign(_, Rvalue::Len(_))));
    assert!(has_len, "No Rvalue::Len in entry block");

    match &entry_block.terminator.kind {
        glyim_mir::TerminatorKind::SwitchInt { discr, .. } => {
            assert!(matches!(discr, Operand::Copy(p) if p.local == LocalIdx::from_raw(1)));
        }
        _ => panic!("Expected SwitchInt terminator"),
    }
}
