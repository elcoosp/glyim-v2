use crate::thir;
use super::common::*;
use glyim_hir::*;
use glyim_type::*;

#[test]
fn assign_to_local() {
    // HIR: x = 5; where x is a Path (we treat as assignable local)
    let lhs = Expr::Path(glyim_hir::Path::from_single(name("x")));
    let rhs = Expr::Literal(Literal::Int(5, None));
    let assign_expr = Expr::Assign { lhs: ExprId::from_raw(0), rhs: ExprId::from_raw(1) };
    let exprs = vec![lhs, rhs, assign_expr];
    let (hir, body_id) = make_single_body_hir(exprs);
    let thir_body = typeck_single_body(&hir, body_id);
    // Assign should become a Stmt::Assign
    assert_eq!(thir_body.stmts.len(), 1);
    match &thir_body.stmts[0] {
        thir::Stmt::Assign { lhs, rhs, span: _ } => {
            // checks that it's an assign
        }
        _ => panic!("Expected Stmt::Assign, got {:?}", thir_body.stmts[0]),
    }
}
