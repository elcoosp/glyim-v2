use super::common::*;
use glyim_hir::*;

#[test]
fn while_loop_bool_condition() {
    let mut exprs = Vec::new();
    exprs.push(Expr::Literal(Literal::Bool(true)));
    exprs.push(Expr::Block { stmts: Vec::new(), tail: None });
    exprs.push(Expr::While { cond: ExprId::from_raw(0), body: ExprId::from_raw(1) });

    let (hir, body_id) = make_single_body_hir(exprs);
    let thir_body = typeck_single_body(&hir, body_id);
    // First two expressions are not tail, so they become stmts
    assert_eq!(thir_body.stmts.len(), 2);
}
