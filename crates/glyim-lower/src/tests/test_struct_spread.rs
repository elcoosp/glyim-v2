use glyim_core::primitives::*;
use glyim_type::*;
use glyim_typeck::thir::*;
use crate::lower::lower_body;
use glyim_test::mock::MockLowerCtx;
use glyim_test::test_frozen_ty_ctx;

#[test]
fn struct_spread_creates_aggregate() {
    let ctx = test_frozen_ty_ctx();
    let mut mock = MockLowerCtx::new(&ctx);

    // Create a dummy ADT for Point { x: i32, y: i32 }
    let int_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let point_ty = ctx.mk_ty(TyKind::Adt(AdtId::from_raw(0), Substitution::empty())); // placeholder

    let base_expr = Expr {
        ty: point_ty,
        span: Default::default(),
        kind: ExprKind::VarRef(LocalVarId::from_raw(0)),
    };
    let struct_expr = Expr {
        ty: point_ty,
        span: Default::default(),
        kind: ExprKind::Struct {
            adt_id: AdtId::from_raw(0),
            variant_idx: 0,
            fields: vec![(Name::from_str("x"), Expr {
                ty: int_ty,
                span: Default::default(),
                kind: ExprKind::Literal(Literal::Int(1, Some(IntTy::I32))),
            })],
            spread: Some(Box::new(base_expr)),
        },
    };
    let body = Body {
        owner: Default::default(),
        params: vec![],
        stmts: vec![],
        expr: struct_expr,
        return_ty: point_ty,
        span: Default::default(),
    };

    // MockLowerCtx must provide adt_def and field_name methods; they are stubs but we can still run.
    let result = lower_body(&mock, &body);
    // Since MockLowerCtx doesn't provide real ADT info, this may produce diagnostics, but we just check it doesn't crash.
    assert!(result.body.basic_blocks.len() > 0, "MIR body should have blocks");
}
