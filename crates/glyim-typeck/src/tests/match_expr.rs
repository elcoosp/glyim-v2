use super::common::*;
use glyim_hir::*;

#[test]
fn match_integer_with_wildcard() {
    let mut exprs = Vec::new();
    let mut pats = Vec::new();
    exprs.push(Expr::Literal(Literal::Int(42, None)));
    exprs.push(Expr::Literal(Literal::Int(0, None)));
    pats.push(Pat::Wild);
    let arm = MatchArm {
        pat: PatId::from_raw(0),
        guard: None,
        body: ExprId::from_raw(1),
    };
    exprs.push(Expr::Match {
        scrutinee: ExprId::from_raw(0),
        arms: vec![arm],
    });

    let (mut hir, body_id) = make_single_body_hir(exprs);
    let body = &mut hir.bodies[body_id];
    body.pats = {
        let mut p = glyim_core::arena::IndexVec::new();
        for pat in pats {
            p.push(pat);
        }
        p
    };

    let thir_body = typeck_single_body(&hir, body_id);
    assert_eq!(thir_body.stmts.len(), 2);
}
