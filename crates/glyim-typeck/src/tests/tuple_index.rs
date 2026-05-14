use crate::thir;
use super::common::*;
use glyim_hir::*;
use glyim_type::*;

#[test]
fn tuple_index() {
    // HIR: (1, true).0  -- but we don't have tuple field access in HIR? Use Index expr.
    // Index expr: base[integer index]. For tuple, Index might be used.
    // We'll use Expr::Index { base: tuple, index: literal 0 }.
    let tuple_expr = Expr::Tuple(vec![
        Expr::Literal(Literal::Int(1, None)),
        Expr::Literal(Literal::Bool(true)),
    ]);
    let index_expr = Expr::Literal(Literal::Int(0, None));
    let index_op = Expr::Index { base: ExprId::from_raw(0), index: ExprId::from_raw(1) };
    let exprs = vec![tuple_expr, index_expr, index_op];
    let (hir, body_id) = make_single_body_hir(exprs);
    let thir_body = typeck_single_body(&hir, body_id);
    assert_eq!(thir_body.stmts.len(), 1);
    match &thir_body.stmts[0] {
        thir::Stmt::Expr { expr } => {
            match &expr.kind {
                thir::ExprKind::Index { base, index } => {
                    // base and index are present
                }
                _ => panic!("Expected Index, got {:?}", expr.kind),
            }
        }
        _ => panic!("Expected Stmt::Expr"),
    }
}
