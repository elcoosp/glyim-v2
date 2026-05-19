use super::common::*;
use super::common::*;
use super::common::*;
use glyim_hir::*;

#[test]
fn cast_i32_to_f64() {
    let mut exprs = Vec::new();
    exprs.push(Expr::Literal(Literal::Int(42, None)));
    exprs.push(Expr::Cast {
        expr: ExprId::from_raw(0),
        ty: TypeRef::Path(glyim_hir::Path::from_single(name("f64"))),
    });

    let (hir, body_id) = make_single_body_hir(exprs);
    let thir_body = typeck_single_body(&hir, body_id);
    assert_eq!(thir_body.stmts.len(), 1);
}
