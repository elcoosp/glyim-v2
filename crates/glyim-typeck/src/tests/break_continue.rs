use super::common::*;
use super::common::*;
use super::common::*;
use glyim_hir::*;

#[test]
fn break_and_continue_inside_loop() {
    let mut exprs = Vec::new();
    exprs.push(Expr::Break { value: None });
    exprs.push(Expr::Continue);
    exprs.push(Expr::Block {
        stmts: vec![ExprId::from_raw(0), ExprId::from_raw(1)],
        tail: None,
    });
    exprs.push(Expr::Loop {
        body: ExprId::from_raw(2),
    });

    let (hir, body_id) = make_single_body_hir(exprs);
    let thir_body = typeck_single_body(&hir, body_id);
    // 3 stmts: Break expr, Continue expr, Block expr; Loop is tail -> 0 stmts? Wait:
    // Loop is tail, so it's not a stmt. But Break and Continue are also tail? No, only last expr is tail. In our vector, exprs[0] (Break) pos 0, exprs[1] (Continue) pos 1, exprs[2] (Block) pos 2, exprs[3] (Loop) pos 3. Loop is last, so tail. Break and Continue are not tail. So they become stmts. Block is not tail, so stmt. So we expect 3 stmts.
    assert_eq!(thir_body.stmts.len(), 3);
}
