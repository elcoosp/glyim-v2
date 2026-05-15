use crate::lower::lower_body;
use crate::tests::mock_lower_ctx::TestLowerCtx;
use crate::tests::thir_builder::ThirBuilder;
use glyim_core::def_id::AdtId;
use glyim_core::primitives::IntTy;
use glyim_test::{assert_mir, test_ty_ctx};
use glyim_type::*;
use glyim_typeck::thir::{self, ExprKind, Literal};

#[test]
fn struct_literal_does_not_panic() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let adt_id = AdtId::from_raw(0);
    let subst = ctx_mut.intern_substitution(vec![]);
    let struct_ty = ctx_mut.mk_adt(adt_id, subst);
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();
    let mock = TestLowerCtx { ty_ctx: &ctx };

    let b = ThirBuilder::new(struct_ty, interner);
    let struct_expr = b.expr(
        ExprKind::Struct {
            adt_id,
            variant_idx: 0,
            fields: vec![
                (
                    b.make_name("x"),
                    b.expr(ExprKind::Literal(Literal::Int(1, None)), i32_ty),
                ),
                (
                    b.make_name("y"),
                    b.expr(ExprKind::Literal(Literal::Int(2, None)), i32_ty),
                ),
            ],
            spread: None,
        },
        struct_ty,
    );
    let body = b.into_body(vec![thir::Stmt::Expr { expr: struct_expr }], vec![]);
    let result = lower_body(&mock, &body);
    // Struct literal stubbed; should not panic
    assert_mir(&ctx, &result.body).block_count(1);
}

#[test]
fn struct_literal_with_spread_does_not_panic() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let adt_id = AdtId::from_raw(1);
    let subst = ctx_mut.intern_substitution(vec![]);
    let struct_ty = ctx_mut.mk_adt(adt_id, subst);
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();
    let mock = TestLowerCtx { ty_ctx: &ctx };

    let b = ThirBuilder::new(struct_ty, interner);
    let base = b.expr(ExprKind::Literal(Literal::Int(0, None)), struct_ty);
    let struct_expr = b.expr(
        ExprKind::Struct {
            adt_id,
            variant_idx: 0,
            fields: vec![(
                b.make_name("x"),
                b.expr(ExprKind::Literal(Literal::Int(42, None)), i32_ty),
            )],
            spread: Some(Box::new(base)),
        },
        struct_ty,
    );
    let body = b.into_body(vec![thir::Stmt::Expr { expr: struct_expr }], vec![]);
    let result = lower_body(&mock, &body);
    assert_mir(&ctx, &result.body).block_count(1);
}
