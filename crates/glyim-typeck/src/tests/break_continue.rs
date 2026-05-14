use crate::thir;
use super::common::*;
use glyim_hir::*;
use glyim_type::*;

#[test]
fn break_and_continue_inside_loop() {
    // HIR: loop { break; continue; }
    // We'll create a block with two statements: break and continue.
    // The Loop expression itself contains a body block that includes break and continue.
    // Build break and continue as Expr::Break { value: None } and Expr::Continue.
    let break_expr = Expr::Break { value: None };
    let continue_expr = Expr::Continue;
    // Body of loop: block containing two statements (break and continue)
    // We need to put them inside a Block.
    let inner_block = Expr::Block {
        stmts: vec![ExprId::from_raw(0), ExprId::from_raw(1)],
        tail: None,
    };
    let loop_expr = Expr::Loop { body: ExprId::from_raw(2) };
    // order: break(0), continue(1), inner_block(2), loop(3)
    let exprs = vec![break_expr, continue_expr, inner_block, loop_expr];
    let (mut hir, body_id) = make_single_body_hir(exprs);
    // We'll need to set up the stmts references correctly; currently inner_block stmts refer to exprs 0 and 1, which is fine.
    let thir_body = typeck_single_body(&hir, body_id);
    // Expect one statement: the loop expression
    assert_eq!(thir_body.stmts.len(), 1);
    match &thir_body.stmts[0] {
        thir::Stmt::Expr { expr } => {
            match &expr.kind {
                thir::ExprKind::Loop { body: loop_body } => {
                    // check that loop body contains break and continue
                    // For now, just verify it's a Block with two stmts that are Break and Continue.
                    match &loop_body.kind {
                        thir::ExprKind::Block { stmts, tail: _ } => {
                            assert_eq!(stmts.len(), 2);
                            match &stmts[0] {
                                thir::Stmt::Expr { expr: break_expr } => {
                                    match &break_expr.kind {
                                        thir::ExprKind::Break { value: None } => {}
                                        _ => panic!("Expected Break"),
                                    }
                                }
                                _ => panic!("Expected Stmt::Expr"),
                            }
                            match &stmts[1] {
                                thir::Stmt::Expr { expr: cont_expr } => {
                                    match &cont_expr.kind {
                                        thir::ExprKind::Continue => {}
                                        _ => panic!("Expected Continue"),
                                    }
                                }
                                _ => panic!("Expected Stmt::Expr"),
                            }
                        }
                        _ => panic!("Expected Block"),
                    }
                }
                _ => panic!("Expected Loop, got {:?}", expr.kind),
            }
        }
        _ => panic!("Expected Stmt::Expr"),
    }
}
