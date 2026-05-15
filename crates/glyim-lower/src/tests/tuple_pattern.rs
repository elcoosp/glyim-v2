use crate::lower::lower_body;
use crate::tests::mock_lower_ctx::TestLowerCtx;
use crate::tests::thir_builder::ThirBuilder;
use glyim_core::primitives::IntTy;
use glyim_test::{assert_mir, test_ty_ctx};
use glyim_type::*;
use glyim_typeck::thir::{self, ExprKind, Literal};

#[test]
fn let_with_tuple_pattern_no_panic() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let subst = ctx_mut.intern_substitution(vec![
        glyim_type::GenericArg::Ty(i32_ty),
        glyim_type::GenericArg::Ty(i32_ty),
    ]);
    let tuple_ty = ctx_mut.mk_ty(TyKind::Tuple(subst));
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();
    let mock = TestLowerCtx { ty_ctx: &ctx };

    let b = ThirBuilder::new(Ty::UNIT, interner);
    let init = Some(b.expr(
        ExprKind::Tuple(vec![
            b.expr(ExprKind::Literal(Literal::Int(1, None)), i32_ty),
            b.expr(ExprKind::Literal(Literal::Int(2, None)), i32_ty),
        ]),
        tuple_ty,
    ));
    let pat = b.pat(thir::PatternKind::Wild, tuple_ty);
    let stmts = vec![thir::Stmt::Let {
        name: b.make_name("_tup"),
        ty: tuple_ty,
        pat,
        init,
        span: glyim_span::Span::DUMMY,
    }];
    let body = b.into_body(stmts, vec![]);
    let result = lower_body(&mock, &body);
    assert!(result.diagnostics.is_empty());
    assert_mir(&ctx, &result.body).block_count(1);
}
