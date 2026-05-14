use crate::thir;
use super::common::*;
use glyim_hir::*;
use glyim_type::*;

#[test]
fn function_call_with_args() {
    // HIR: f(1, true) where f is a path expression (unresolved).
    let func_expr = Expr::Path(glyim_hir::Path::from_single(name("f")));
    let arg1 = Expr::Literal(Literal::Int(1, None));
    let arg2 = Expr::Literal(Literal::Bool(true));
    let call_expr = Expr::Call { func: ExprId::from_raw(0), args: vec![ExprId::from_raw(1), ExprId::from_raw(2)] };
    let exprs = vec![func_expr, arg1, arg2, call_expr];
    let (hir, body_id) = make_single_body_hir(exprs);
    let thir_body = typeck_single_body(&hir, body_id);
    assert_eq!(thir_body.stmts.len(), 1);
    match &thir_body.stmts[0] {
        thir::Stmt::Expr { expr } => {
            match &expr.kind {
                thir::ExprKind::Call { func, args } => {
                    assert_eq!(args.len(), 2);
                }
                _ => panic!("Expected Call, got {:?}", expr.kind),
            }
        }
        _ => panic!("Expected Stmt::Expr"),
    }
}
