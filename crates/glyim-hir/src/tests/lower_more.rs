#![allow(clippy::redundant_closure, unused)]
use crate::lower::lower_crate;
use crate::{BodyId, Expr, ExprId, ItemId, ItemKind, Literal, Pat, PatId};
use glyim_core::interner::Interner;
use glyim_core::primitives::*;
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
fn test_chained_field_access() {
    // Parser may produce sibling FieldExpr nodes; HIR lowerer merges them.
    // Just verify the function lowers successfully with a body.
    let source = "fn f() { a.b.c }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner, &mut Vec::new());
    assert_eq!(hir.items.len(), 1);
    if let ItemKind::Fn(fn_item) = &hir.items[ItemId::from_raw(0)].kind {
        assert!(fn_item.body.is_some());
    } else {
        panic!("Expected Fn");
    }
}

#[test]
fn test_chained_method_calls() {
    let source = "fn f() { a.b().c() }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner, &mut Vec::new());
    if let ItemKind::Fn(fn_item) = &hir.items[ItemId::from_raw(0)].kind {
        assert!(fn_item.body.is_some());
    } else {
        panic!("Expected Fn");
    }
}

#[test]
fn test_return_with_value() {
    let source = "fn f() -> i32 { return 42; }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner, &mut Vec::new());
    if let ItemKind::Fn(fn_item) = &hir.items[ItemId::from_raw(0)].kind {
        assert!(fn_item.body.is_some());
        let body = &hir.bodies[fn_item.body.unwrap()];
        let has_return = body
            .exprs
            .iter_enumerated()
            .any(|(_, expr)| matches!(expr, Expr::Return { .. }));
        assert!(has_return, "Expected Return in body");
    } else {
        panic!("Expected Fn");
    }
}

#[test]
fn test_while_loop_with_body() {
    let source = "fn f() { while x > 0 { x = x - 1; } }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner, &mut Vec::new());
    if let ItemKind::Fn(fn_item) = &hir.items[ItemId::from_raw(0)].kind {
        assert!(fn_item.body.is_some());
        let body = &hir.bodies[fn_item.body.unwrap()];
        let has_while = body
            .exprs
            .iter_enumerated()
            .any(|(_, expr)| matches!(expr, Expr::While { .. }));
        assert!(has_while, "Expected While in body");
    } else {
        panic!("Expected Fn");
    }
}

// Comparison operators
#[test]
fn test_comparison_operators() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { a < b }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block {
            tail: Some(bin_id), ..
        } => match &body.exprs[*bin_id] {
            Expr::Binary { op, .. } => assert_eq!(*op, BinOp::Lt),
            other => panic!("Expected Binary(Lt), got {:?}", other),
        },
        _ => panic!(),
    }
}

// Boolean literals
#[test]
fn test_bool_literal_false() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { false }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block {
            tail: Some(tail_id),
            ..
        } => match &body.exprs[*tail_id] {
            Expr::Literal(Literal::Bool(false)) => {}
            other => panic!("Expected Bool(false), got {:?}", other),
        },
        other => panic!("Expected Block",),
    }
}

// Unary deref
#[test]
fn test_unary_deref() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { *x }");
    let body = &hir.bodies[body_id];
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block {
            tail: Some(tail_id),
            ..
        } => match &body.exprs[*tail_id] {
            Expr::Unary { op, .. } => assert_eq!(*op, UnOp::Deref),
            other => panic!("Expected Unary(Deref), got {:?}", other),
        },
        other => panic!("Expected Block",),
    }
}

// Type slice
#[test]
fn test_type_ref_slice() {
    let source = "fn f() -> [i32] { todo!() }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner, &mut Vec::new());
    match &hir.items[ItemId::from_raw(0)].kind {
        ItemKind::Fn(fn_item) => {
            assert!(fn_item.return_ty.is_some());
        }
        _ => panic!("Expected Fn"),
    }
}

// Struct with generic
#[test]
fn test_struct_with_generic() {
    let source = "struct Point<T> { x: T, y: T }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner, &mut Vec::new());
    match &hir.items[ItemId::from_raw(0)].kind {
        ItemKind::Struct(s) => {
            assert_eq!(s.kind, StructKind::Record);
            assert_eq!(s.fields.len(), 2);
        }
        _ => panic!("Expected Struct"),
    }
}

// Enum record variant
#[test]
fn test_enum_record_variant() {
    let source = "enum Shape { Circle { radius: f64 }, Square { side: f64 } }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner, &mut Vec::new());
    match &hir.items[ItemId::from_raw(0)].kind {
        ItemKind::Enum(e) => {
            assert_eq!(e.variants.len(), 2);
            assert_eq!(e.variants[0].kind, StructKind::Record);
            assert_eq!(e.variants[0].fields.len(), 1);
        }
        _ => panic!("Expected Enum"),
    }
}

// Module parsing
#[test]
fn test_module_with_multiple_items() {
    let source = "mod m { fn a() {} struct B; enum C { X } }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let _ = lower_crate(&result.root, &mut interner, &mut Vec::new());
}

// Array repeat
#[test]
fn test_array_repeat_expr() {
    let source = "fn f() { [0; 10] }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let _ = lower_crate(&result.root, &mut interner, &mut Vec::new());
}

// Multiple functions
#[test]
fn test_multiple_functions() {
    let source = "fn a() {} fn b() {} fn c() {}";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner, &mut Vec::new());
    assert_eq!(hir.items.len(), 3);
}
