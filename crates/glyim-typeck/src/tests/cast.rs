use crate::thir;
use super::common::*;
use glyim_hir::*;
use glyim_type::*;

#[test]
fn cast_i32_to_f64() {
    // HIR: 42 as f64
    let src_expr = Expr::Literal(Literal::Int(42, None));
    // Cast expr expects ty: TypeRef. We'll use Path for f64.
    let ty_ref = TypeRef::Path(glyim_hir::Path::from_single(name("f64")));
    let cast_expr = Expr::Cast { expr: ExprId::from_raw(0), ty: ty_ref };
    let exprs = vec![src_expr, cast_expr];
    let (hir, body_id) = make_single_body_hir(exprs);
    let thir_body = typeck_single_body(&hir, body_id);
    assert_eq!(thir_body.stmts.len(), 1);
    match &thir_body.stmts[0] {
        thir::Stmt::Expr { expr } => {
            match &expr.kind {
                thir::ExprKind::Cast { expr: _ } => {} // OK
                _ => panic!("Expected Cast, got {:?}", expr.kind),
            }
        }
        _ => panic!("Expected Stmt::Expr"),
    }
}
