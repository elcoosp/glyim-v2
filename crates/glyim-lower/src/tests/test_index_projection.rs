use glyim_core::primitives::*;
use glyim_type::*;
use glyim_typeck::thir::*;
use crate::lower::lower_body;
use glyim_test::mock::MockLowerCtx;
use glyim_test::test_frozen_ty_ctx;

#[test]
fn index_projection_created() {
    let ctx = test_frozen_ty_ctx();
    let mut mock = MockLowerCtx::new(&ctx);

    // Build a THIR body: let arr = [10,20,30]; let _ = arr[1];
    // Simplified: just an Index expression
    let int_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let arr_ty = ctx.mk_ty(TyKind::Array(int_ty, Const::from_usize(3).unwrap()));

    let arr_expr = Expr {
        ty: arr_ty,
        span: Default::default(),
        kind: ExprKind::VarRef(LocalVarId::from_raw(0)),
    };
    let index_expr = Expr {
        ty: int_ty,
        span: Default::default(),
        kind: ExprKind::Literal(Literal::Int(1, Some(IntTy::I32))),
    };
    let index_expr = Expr {
        ty: int_ty,
        span: Default::default(),
        kind: ExprKind::Index {
            base: Box::new(arr_expr),
            index: Box::new(index_expr),
        },
    };
    let body = Body {
        owner: Default::default(),
        params: vec![],
        stmts: vec![],
        expr: index_expr,
        return_ty: int_ty,
        span: Default::default(),
    };

    let result = lower_body(&mock, &body);
    assert!(result.diagnostics.is_empty(), "Lowering produced errors: {:?}", result.diagnostics);

    // Verify the MIR contains an Index projection
    let mut found = false;
    for block in result.body.basic_blocks.iter() {
        for stmt in &block.statements {
            if let glyim_mir::StatementKind::Assign(place, _) = &stmt.kind {
                for proj in place.projection.iter() {
                    if matches!(proj, glyim_mir::ProjectionElem::Index(_)) {
                        found = true;
                    }
                }
            }
        }
    }
    assert!(found, "No Index projection found in MIR");
}
