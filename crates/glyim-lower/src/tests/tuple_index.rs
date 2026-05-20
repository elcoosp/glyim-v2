use crate::lower::lower_body;
use crate::tests::mock_lower_ctx::TestLowerCtx;
use crate::tests::thir_builder::ThirBuilder;
use glyim_core::primitives::IntTy;
use glyim_test::test_ty_ctx;
use glyim_type::*;
use glyim_typeck::thir::{self, ExprKind, Literal};

#[test]
fn tuple_index_to_field_projection() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let subst = ctx_mut.intern_substitution(vec![
        glyim_type::GenericArg::Ty(i32_ty),
        glyim_type::GenericArg::Ty(i32_ty),
    ]);
    let tuple_ty = ctx_mut.mk_ty(TyKind::Tuple(subst));
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();
    let mock = TestLowerCtx::new(&ctx);

    let mut b = ThirBuilder::new(Ty::UNIT, interner);
    let mut stmts = Vec::new();
    b.add_let_binding("t", tuple_ty, None, &mut stmts);

    let idx_expr = b.expr(
        ExprKind::Index {
            base: Box::new(b.var_ref_expr("t", tuple_ty)),
            index: Box::new(b.expr(ExprKind::Literal(Literal::Int(0, None)), i32_ty)),
        },
        Ty::UNIT,
    );
    stmts.push(thir::Stmt::Expr { expr: idx_expr });

    let body = b.into_body(stmts, vec![]);
    let result = lower_body(&mock, &body);
    assert!(result.diagnostics.is_empty());
}
