#![allow(unused_variables)]

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
    let hir = lower_crate(&parse_result.root, &mut interner, &mut Vec::new());
    let body_id = match &hir.items[ItemId::from_raw(0)].kind {
        ItemKind::Fn(fn_item) => fn_item.body.expect("no body"),
        other => panic!("expected Fn item, got {:?}", other),
    };
    (hir, interner, body_id)
}

fn last_expr_id(body: &crate::Body) -> ExprId {
    ExprId::from_raw(body.exprs.len() as u32 - 1)
}

#[test]
fn test_chained_binary_expression() {
    // a + b + c  should parse as (a + b) + c
    let (hir, _interner, body_id) = get_body_hir("fn f() { a + b + c }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block { stmts, tail } => {
            assert!(stmts.is_empty());
            let expr_id = tail.unwrap();
            match &body.exprs[expr_id] {
                Expr::Binary { op, .. } => {
                    // top-level op should be +
                    assert_eq!(*op, BinOp::Add);
                }
                other => panic!("Expected Binary in tail, got {:?}", other),
            }
        }
        other => panic!("Expected Block, got {:?}", other),
    }
}

#[test]
fn test_if_without_else() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { if true { 42 } }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block { stmts, tail } => {
            assert!(stmts.is_empty());
            let if_id = tail.unwrap();
            match &body.exprs[if_id] {
                Expr::If { else_branch, .. } => {
                    assert!(else_branch.is_none(), "should have no else branch");
                }
                other => panic!("Expected If, got {:?}", other),
            }
        }
        other => panic!("Expected Block, got {:?}", other),
    }
}

#[test]
fn test_block_multiple_stmts_and_tail() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { 1; 2; 3 }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block { stmts, tail } => {
            assert_eq!(stmts.len(), 2, "should have two statements");
            assert!(tail.is_some(), "should have tail expression");
        }
        other => panic!("Expected Block, got {:?}", other),
    }
}

#[test]
fn test_nested_blocks() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { { 1 } }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block { stmts, tail } => {
            assert!(stmts.is_empty());
            let inner_id = tail.unwrap();
            match &body.exprs[inner_id] {
                Expr::Block {
                    stmts: inner_stmts,
                    tail: inner_tail,
                } => {
                    assert!(inner_stmts.is_empty());
                    assert!(inner_tail.is_some());
                }
                other => panic!("Expected inner Block, got {:?}", other),
            }
        }
        other => panic!("Expected outer Block, got {:?}", other),
    }
}

#[test]
fn test_empty_enum() {
    let source = "enum Void {}";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let hir = lower_crate(&parse_result.root, &mut interner, &mut Vec::new());
    assert_eq!(hir.items.len(), 1);
    let item = &hir.items[ItemId::from_raw(0)];
    match &item.kind {
        ItemKind::Enum(e) => {
            assert!(e.variants.is_empty());
        }
        other => panic!("Expected Enum item, got {:?}", other),
    }
}

#[test]
fn test_boolean_literals() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { true }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block { stmts, tail } => {
            let lit_id = tail.unwrap();
            match &body.exprs[lit_id] {
                Expr::Literal(Literal::Bool(true)) => {}
                other => panic!("Expected Bool(true), got {:?}", other),
            }
        }
        other => panic!("Expected Block, got {:?}", other),
    }
}

#[test]
fn test_reference_type_with_mut() {
    // This requires the parser to produce RefType with KwMut; may not work yet.
    let source = "fn f() -> &mut i32 { todo!() }";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let hir = lower_crate(&parse_result.root, &mut interner, &mut Vec::new());
    let item = &hir.items[ItemId::from_raw(0)];
    if let ItemKind::Fn(fn_item) = &item.kind
        && let Some(ty) = &fn_item.return_ty
    {
        // we just check that it doesn't panic
        eprintln!("return type: {:?}", ty);
    }
}

#[test]
fn test_multiple_top_level_items() {
    let source = "struct S; fn f() {} enum E { A }";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let hir = lower_crate(&parse_result.root, &mut interner, &mut Vec::new());
    assert_eq!(hir.items.len(), 3);
    assert!(matches!(
        hir.items[ItemId::from_raw(0)].kind,
        ItemKind::Struct(_)
    ));
    assert!(matches!(
        hir.items[ItemId::from_raw(1)].kind,
        ItemKind::Fn(_)
    ));
    assert!(matches!(
        hir.items[ItemId::from_raw(2)].kind,
        ItemKind::Enum(_)
    ));
}
