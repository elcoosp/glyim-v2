use crate::thir;
use super::common::*;
use glyim_hir::*;
use glyim_type::*;

#[test]
fn struct_field_access() {
    // HIR: expr x.field, where x is a local binding we haven't defined.
    // We'll simplify by using a Path expression (unresolved) - it will be error,
    // but the checker will still produce Field thir node (maybe with errors).
    // We can test structural generation.
    let name_x = name("x");
    let name_field = name("field");
    // Create receiver: Path to x
    let receiver_expr = Expr::Path(glyim_hir::Path::from_single(name_x));
    // Create Field expr
    let field_expr = Expr::Field { receiver: ExprId::from_raw(0), field: name_field };
    let exprs = vec![receiver_expr, field_expr];
    let (hir, body_id) = make_single_body_hir(exprs);
    let thir_body = typeck_single_body(&hir, body_id);
    // The field expr should produce a thir::ExprKind::Field
    assert_eq!(thir_body.stmts.len(), 1);
    match &thir_body.stmts[0] {
        thir::Stmt::Expr { expr } => {
            match &expr.kind {
                thir::ExprKind::Field { receiver, field, ty: _ } => {
                    assert_eq!(*field, name_field);
                }
                _ => panic!("Expected Field, got {:?}", expr.kind),
            }
        }
        _ => panic!("Expected Stmt::Expr"),
    }
}
