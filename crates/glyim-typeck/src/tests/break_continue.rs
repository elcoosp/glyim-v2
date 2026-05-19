use super::common::*;
use glyim_hir::*;

#[test]
fn break_and_continue_inside_loop() {
    let mut exprs = Vec::new();
    exprs.push(Expr::Break { value: None });
    exprs.push(Expr::Continue);
    exprs.push(Expr::Block {
        stmts: vec![ExprId::from_raw(0), ExprId::from_raw(1)],
        tail: None,
    });
    exprs.push(Expr::Loop {
        body: ExprId::from_raw(2),
    });

    let (hir, body_id) = make_single_body_hir(exprs);
    let thir_body = typeck_single_body(&hir, body_id);
    assert_eq!(thir_body.stmts.len(), 4);
}
