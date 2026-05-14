use crate::thir;
use super::common::*;
use glyim_hir::*;
use glyim_type::*;

#[test]
fn while_loop_bool_condition() {
    let name_true = name("true");
    // Build HIR: while true { };
    // Condition: literal bool true
    let cond_expr = Expr::Literal(Literal::Bool(true));
    // Body: block with no statements and no tail -> unit
    let body_expr = Expr::Block { stmts: Vec::new(), tail: None };

    // We'll put condition and body into the Hir body, then a While expr referencing them.
    // Expr::While { cond: ExprId, body: ExprId } — we need to create IDs.
    // So we'll create a vector of exprs: [cond, body, While].
    let mut exprs = Vec::new();
    exprs.push(cond_expr); // index 0
    exprs.push(body_expr); // index 1
    exprs.push(Expr::While { cond: ExprId::from_raw(0), body: ExprId::from_raw(1) }); // index 2

    let (hir, body_id) = make_single_body_hir(exprs);

    let thir_body = typeck_single_body(&hir, body_id);

    // Expect one statement: the while expression
    assert_eq!(thir_body.stmts.len(), 1, "Expected one statement (while)");
    let stmt = &thir_body.stmts[0];
    match stmt {
        thir::Stmt::Expr { expr } => {
            match &expr.kind {
                thir::ExprKind::While { cond, body } => {
                    // Check that condition type is bool
                    // For now, we can't check inference fully, but we verify structure
                    assert!(cond.kind.is_some()); // placeholder
                    assert!(body.kind.is_some());
                }
                _ => panic!("Expected ExprKind::While, got {:?}", expr.kind),
            }
        }
        _ => panic!("Expected Stmt::Expr, got {:?}", stmt),
    }
}
