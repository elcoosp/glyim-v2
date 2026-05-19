use super::common::*;
use glyim_hir::*;

#[test]
fn tuple_index() {
    let mut exprs = Vec::new();
    exprs.push(Expr::Literal(Literal::Int(1, None)));
    exprs.push(Expr::Literal(Literal::Bool(true)));
    exprs.push(Expr::Tuple(vec![ExprId::from_raw(0), ExprId::from_raw(1)]));
    exprs.push(Expr::Literal(Literal::Int(0, None)));
    exprs.push(Expr::Index {
        base: ExprId::from_raw(2),
        index: ExprId::from_raw(3),
    });
    let (hir, body_id) = make_single_body_hir(exprs);
    let thir_body = typeck_single_body(&hir, body_id);
    assert_eq!(thir_body.stmts.len(), 4);
}
