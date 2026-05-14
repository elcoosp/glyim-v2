use crate::thir;
use super::common::*;
use glyim_hir::*;
use glyim_type::*;

#[test]
fn array_literal_and_index() {
    // HIR: [1,2,3][1]  -- array literal and indexing
    let array_expr = Expr::Array(vec![
        Expr::Literal(Literal::Int(1, None)),
        Expr::Literal(Literal::Int(2, None)),
        Expr::Literal(Literal::Int(3, None)),
    ]);
    let index_expr = Expr::Literal(Literal::Int(1, None));
    let index_op = Expr::Index { base: ExprId::from_raw(0), index: ExprId::from_raw(1) };
    let exprs = vec![array_expr, index_expr, index_op];
    let (hir, body_id) = make_single_body_hir(exprs);
    let thir_body = typeck_single_body(&hir, body_id);
    assert_eq!(thir_body.stmts.len(), 1);
    match &thir_body.stmts[0] {
        thir::Stmt::Expr { expr } => {
            match &expr.kind {
                thir::ExprKind::Index { .. } => {}
                _ => panic!("Expected Index, got {:?}", expr.kind),
            }
        }
        _ => panic!("Expected Stmt::Expr"),
    }
}
