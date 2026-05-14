use crate::thir;
use super::common::*;
use glyim_hir::*;
use glyim_type::*;

#[test]
fn return_expression() {
    // HIR: return 42;
    let value_expr = Expr::Literal(Literal::Int(42, None));
    let return_expr = Expr::Return { value: Some(ExprId::from_raw(0)) };
    let exprs = vec![value_expr, return_expr];
    let (hir, body_id) = make_single_body_hir(exprs);
    let thir_body = typeck_single_body(&hir, body_id);
    // Return should become Stmt::Return
    assert_eq!(thir_body.stmts.len(), 1);
    match &thir_body.stmts[0] {
        thir::Stmt::Return { value, span: _ } => {
            assert!(value.is_some());
        }
        _ => panic!("Expected Stmt::Return, got {:?}", thir_body.stmts[0]),
    }
}
