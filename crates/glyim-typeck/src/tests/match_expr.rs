use crate::thir;
use super::common::*;
use glyim_hir::*;
use glyim_type::*;

#[test]
fn match_integer_with_wildcard() {
    // Build HIR: match 42 { _ => 0 }
    let scrutinee = Expr::Literal(Literal::Int(42, None));
    // arm: wildcard pattern -> literal 0
    let wild_pat = Pat::Wild;
    let body_expr = Expr::Literal(Literal::Int(0, None));
    // need to create PatId and ExprId
    let mut exprs = Vec::new();
    let mut pats = Vec::new();

    // push scrutinee expr (index 0)
    exprs.push(scrutinee);
    // push body expr (index 1)
    exprs.push(body_expr);
    // push wild pattern (index 0)
    pats.push(wild_pat);

    let match_arm = MatchArm {
        pat: PatId::from_raw(0),
        guard: None,
        body: ExprId::from_raw(1),
    };
    exprs.push(Expr::Match {
        scrutinee: ExprId::from_raw(0),
        arms: vec![match_arm],
    });

    let (mut hir, body_id) = make_single_body_hir(exprs);
    // We need to add pats to the body. The helper currently doesn't allow pats.
    // Modify body in hir to include pats.
    let body = &mut hir.bodies[body_id];
    for pat in pats {
        body.pats.push(pat);
    }

    let thir_body = typeck_single_body(&hir, body_id);

    // Expect one statement: the match expression
    assert_eq!(thir_body.stmts.len(), 1);
    let stmt = &thir_body.stmts[0];
    match stmt {
        thir::Stmt::Expr { expr } => {
            match &expr.kind {
                thir::ExprKind::Match { scrutinee, arms } => {
                    assert_eq!(arms.len(), 1);
                }
                _ => panic!("Expected ExprKind::Match, got {:?}", expr.kind),
            }
        }
        _ => panic!("Expected Stmt::Expr, got {:?}", stmt),
    }
}
