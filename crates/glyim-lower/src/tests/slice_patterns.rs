use crate::tests::support::MockLowerCtx;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::primitives::{IntTy, Mutability};
use glyim_mir::{BasicBlockIdx, LocalIdx, Operand, Rvalue, StatementKind};
use glyim_span::Span;
use glyim_type::{TyCtxMut, TyKind};
use glyim_typeck::thir::{
    self, Body, Expr, ExprKind, Literal, MatchArm, Pattern, PatternKind, Stmt,
};

fn make_slice_pattern_body(ctx_mut: &mut TyCtxMut) -> Body {
    let elem_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let slice_ty = ctx_mut.mk_ty(TyKind::Slice(elem_ty));

    let scrutinee = Expr {
        kind: ExprKind::VarRef(thir::LocalVarId::from_raw(0)),
        ty: slice_ty,
        span: Span::DUMMY,
    };

    let prefix = vec![
        Pattern {
            kind: PatternKind::Binding {
                name: ctx_mut.resolver().intern("a"),
                mutability: Mutability::Not,
                subpattern: None,
            },
            ty: elem_ty,
            span: Span::DUMMY,
        },
        Pattern {
            kind: PatternKind::Binding {
                name: ctx_mut.resolver().intern("b"),
                mutability: Mutability::Not,
                subpattern: None,
            },
            ty: elem_ty,
            span: Span::DUMMY,
        },
        Pattern {
            kind: PatternKind::Binding {
                name: ctx_mut.resolver().intern("c"),
                mutability: Mutability::Not,
                subpattern: None,
            },
            ty: elem_ty,
            span: Span::DUMMY,
        },
    ];
    let pat = Pattern {
        kind: PatternKind::Slice {
            prefix,
            slice: None,
            suffix: vec![],
        },
        ty: slice_ty,
        span: Span::DUMMY,
    };

    let arm_body = Expr {
        kind: ExprKind::Literal(Literal::Unit),
        ty: ctx_mut.unit_ty(),
        span: Span::DUMMY,
    };
    let arm = MatchArm {
        pat,
        guard: None,
        body: arm_body,
    };
    let match_expr = Expr {
        kind: ExprKind::Match {
            scrutinee: Box::new(scrutinee),
            arms: vec![arm],
        },
        ty: ctx_mut.unit_ty(),
        span: Span::DUMMY,
    };
    let stmt = Stmt::Expr { expr: match_expr };
    Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        params: vec![],
        return_ty: ctx_mut.unit_ty(),
        stmts: vec![stmt],
        span: Span::DUMMY,
    }
}

#[test]
fn slice_pattern_lowering_emits_len_and_switch() {
    let mut ctx_mut = glyim_test::test_ty_ctx();
    let body = make_slice_pattern_body(&mut ctx_mut);
    let ctx = ctx_mut.freeze();
    let lower_ctx = MockLowerCtx::new(&ctx);
    let result = crate::lower_body(&lower_ctx, &body);
    assert!(
        result.diagnostics.is_empty(),
        "Lowering produced errors: {:?}",
        result.diagnostics
    );

    let mir_body = result.body;
    let entry_block = &mir_body.basic_blocks[BasicBlockIdx::from_raw(0)];
    let has_len = entry_block
        .statements
        .iter()
        .any(|s| matches!(s.kind, StatementKind::Assign(_, Rvalue::Len(_))));
    assert!(has_len, "No Rvalue::Len in entry block");

    // The SwitchInt must discriminate on the slice *length* computed by the
    // `Rvalue::Len` assignment in the entry block. Recover that target local and
    // assert the switch reads from it.
    let len_target = entry_block
        .statements
        .iter()
        .find_map(|s| match &s.kind {
            StatementKind::Assign(place, Rvalue::Len(_)) => Some(place.local),
            _ => None,
        })
        .expect("Rvalue::Len target local");

    match &entry_block.terminator.kind {
        glyim_mir::TerminatorKind::SwitchInt { discr, .. } => {
            assert!(
                matches!(discr, Operand::Copy(p) if p.local == len_target),
                "SwitchInt should discriminate on the slice length local {:?}, got {:?}",
                len_target,
                discr
            );
        }
        _ => panic!("Expected SwitchInt terminator"),
    }
}
