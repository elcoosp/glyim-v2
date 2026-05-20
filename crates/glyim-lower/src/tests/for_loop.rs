use crate::lower::lower_body;
use crate::tests::mock_lower_ctx::TestLowerCtx;
use crate::tests::thir_builder::ThirBuilder;
use glyim_core::primitives::{IntTy, Mutability};
use glyim_test::test_ty_ctx;
use glyim_type::*;
use glyim_typeck::thir::{self, ExprKind, Literal, Pattern, PatternKind};

#[test]
fn for_loop_does_not_panic() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let usize_ty = ctx_mut.mk_ty(TyKind::Uint(glyim_core::primitives::UintTy::Usize));
    let arr_ty = ctx_mut.mk_ty(TyKind::Array(
        i32_ty,
        glyim_type::Const {
            kind: glyim_type::ConstKind::Int(3),
            ty: usize_ty,
        },
    ));
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();
    let mock = TestLowerCtx::new(&ctx);

    let mut b = ThirBuilder::new(Ty::UNIT, interner);
    let mut stmts = Vec::new();

    let arr_var = b.expr(
        ExprKind::Array(vec![
            b.expr(ExprKind::Literal(Literal::Int(1, None)), i32_ty),
            b.expr(ExprKind::Literal(Literal::Int(2, None)), i32_ty),
        ]),
        arr_ty,
    );
    b.add_let_binding("arr", arr_ty, Some(arr_var), &mut stmts);

    let elem_pat = Pattern {
        kind: PatternKind::Binding {
            name: b.make_name("elem"),
            mutability: Mutability::Not,
            subpattern: None,
        },
        ty: i32_ty,
        span: glyim_span::Span::DUMMY,
    };
    let for_body = b.expr(
        ExprKind::Block {
            stmts: vec![],
            tail: None,
        },
        Ty::UNIT,
    );
    let for_expr = b.expr(
        ExprKind::For {
            pat: Box::new(elem_pat),
            iterable: Box::new(b.var_ref_expr("arr", arr_ty)),
            body: Box::new(for_body),
        },
        Ty::UNIT,
    );
    stmts.push(thir::Stmt::Expr { expr: for_expr });

    let body = b.into_body(stmts, vec![]);
    let result = lower_body(&mock, &body);
    // For-loop lowering now produces a loop structure with multiple blocks
    // (header, body, exit) instead of the old stub that returned 1 block.
    assert!(
        result.body.basic_blocks.len() >= 3,
        "expected at least 3 blocks for for-loop, got {}",
        result.body.basic_blocks.len()
    );
    assert!(
        !result.diagnostics.iter().any(|d| d.is_error()),
        "for-loop lowering should not produce error diagnostics"
    );
}
