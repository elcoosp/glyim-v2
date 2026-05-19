use crate::lower::lower_crate;
use crate::{BodyId, Expr, ExprId, ItemId, ItemKind, Literal};
use glyim_core::interner::Interner;
use glyim_core::primitives::BinOp;
use glyim_frontend::parse_to_syntax;
use glyim_span::FileId;

fn get_body_hir(source: &str) -> (crate::CrateHir, Interner, BodyId) {
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let hir = lower_crate(&parse_result.root, &mut interner);
    let body_id = match &hir.items[ItemId::from_raw(0)].kind {
        ItemKind::Fn(fn_item) => fn_item.body.expect("no body", &mut Vec::new()),
        other => panic!("expected Fn item, got {:?}", other),
    };
    (hir, interner, body_id)
}

fn last_expr_id(body: &crate::Body) -> ExprId {
    ExprId::from_raw(body.exprs.len() as u32 - 1)
}

#[test]
fn test_block_expression() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { 1 + 2; 3 }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block { stmts, tail } => {
            assert!(!stmts.is_empty(), "should have at least one statement");
            assert!(tail.is_some(), "should have tail expression");
        }
        other => panic!("Expected Block, got {:?}", other),
    }
}

#[test]
fn test_binary_expression() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { 1 + 2 }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block { stmts, tail } => {
            assert!(stmts.is_empty(), "should have no statements");
            let bin_id = tail.expect("should have tail");
            match &body.exprs[bin_id] {
                Expr::Binary { op, lhs, rhs } => {
                    assert_eq!(*op, BinOp::Add);
                    match &body.exprs[*lhs] {
                        Expr::Literal(lit) => assert_eq!(*lit, Literal::Int(1, None)),
                        other => panic!("lhs not literal, got {:?}", other),
                    }
                    match &body.exprs[*rhs] {
                        Expr::Literal(lit) => assert_eq!(*lit, Literal::Int(2, None)),
                        other => panic!("rhs not literal, got {:?}", other),
                    }
                }
                other => panic!("Expected Binary in tail, got {:?}", other),
            }
        }
        other => panic!("Expected Block, got {:?}", other),
    }
}

#[test]
fn test_if_else_expression() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { if true { 1 } else { 2 } }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block { stmts, tail } => {
            assert!(stmts.is_empty(), "should have no statements");
            let if_id = tail.expect("should have tail");
            match &body.exprs[if_id] {
                Expr::If {
                    cond: _,
                    then_branch: _,
                    else_branch,
                } => {
                    assert!(else_branch.is_some(), "should have else branch");
                }
                other => panic!("Expected If in tail, got {:?}", other),
            }
        }
        other => panic!("Expected Block, got {:?}", other),
    }
}

#[test]
fn test_path_expression() {
    let (hir, interner, body_id) = get_body_hir("fn f() { x }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block { stmts, tail } => {
            assert!(stmts.is_empty());
            let path_id = tail.expect("should have tail");
            match &body.exprs[path_id] {
                Expr::Path(path) => {
                    assert_eq!(path.segments.len(), 1);
                    assert_eq!(interner.resolve(path.segments[0].name), "x");
                }
                other => panic!("Expected Path in tail, got {:?}", other),
            }
        }
        other => panic!("Expected Block, got {:?}", other),
    }
}

#[test]
fn test_literal_expression() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { 42 }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block { stmts, tail } => {
            assert!(stmts.is_empty());
            let lit_id = tail.expect("should have tail");
            match &body.exprs[lit_id] {
                Expr::Literal(lit) => assert_eq!(*lit, Literal::Int(42, None)),
                other => panic!("Expected Int literal in tail, got {:?}", other),
            }
        }
        other => panic!("Expected Block, got {:?}", other),
    }
}
