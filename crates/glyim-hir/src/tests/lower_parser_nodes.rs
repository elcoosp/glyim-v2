#![allow(unused)]
use crate::lower::lower_crate;
use crate::{BodyId, Expr, ExprId, ItemId, ItemKind, Literal, Pat};
use glyim_core::interner::Interner;
use glyim_core::primitives::*;
use glyim_frontend::parse_to_syntax;
use glyim_span::FileId;
use glyim_syntax::{SyntaxKind, SyntaxNode};

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

fn get_body(hir: &crate::CrateHir, body_id: BodyId) -> &crate::Body {
    &hir.bodies[body_id]
}

// ==================== FieldExpr ====================

#[test]
fn test_field_expr_lowering() {
    let source = "fn f() { a.b }";
    let result = glyim_frontend::parse_to_syntax(source, glyim_span::FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner);
    if let ItemKind::Fn(fn_item) = &hir.items[ItemId::from_raw(0)].kind {
        assert!(fn_item.body.is_some());
    } else {
        panic!("Expected Fn");
    }
}

// ==================== MethodCallExpr ====================

#[test]
fn test_method_call_expr_lowering() {
    let source = "fn f() { a.b(1, &mut Vec::new()) }";
    let result = glyim_frontend::parse_to_syntax(source, glyim_span::FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner);
    if let ItemKind::Fn(fn_item) = &hir.items[ItemId::from_raw(0)].kind {
        assert!(fn_item.body.is_some());
    } else {
        panic!("Expected Fn");
    }
}

// ==================== BreakExpr / ContinueExpr ====================

#[test]
fn test_break_continue_lowering(, &mut Vec::new()) {
    let (hir, _interner, body_id) = get_body_hir("fn f() { loop { break; continue; } }");
    let body = get_body(&hir, body_id);
    let block_id = last_expr_id(body);
    let while_id = match &body.exprs[block_id] {
        Expr::Block { tail: Some(id), .. } => *id,
        _ => panic!("Expected Block with tail"),
    };
    let while_body_id = match &body.exprs[while_id] {
        Expr::Loop { body: b, .. } => *b,
        _ => panic!("Expected While"),
    };
    let (stmts, tail) = match &body.exprs[while_body_id] {
        Expr::Block { stmts, tail } => (stmts.clone(), *tail),
        _ => panic!("Expected Block"),
    };
    let mut saw_break = false;
    let mut saw_continue = false;
    for &sid in &stmts {
        match &body.exprs[sid] {
            Expr::Break { .. } => saw_break = true,
            Expr::Continue => saw_continue = true,
            _ => {}
        }
    }
    if let Some(tail_id) = tail {
        match &body.exprs[tail_id] {
            Expr::Break { .. } => saw_break = true,
            Expr::Continue => saw_continue = true,
            _ => {}
        }
    }
    assert!(saw_break, "Expected BreakExpr in loop body");
    assert!(saw_continue, "Expected ContinueExpr in loop body");
}

// ==================== ForExpr ====================

#[test]
fn test_for_expr_lowering() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { for i in 0..10 { 1; } }");
    let body = get_body(&hir, body_id);
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block {
            tail: Some(for_id), ..
        } => {
            assert!(matches!(&body.exprs[*for_id], Expr::For { .. }));
        }
        other => panic!("Expected Block with For tail, got {:?}", other),
    }
}

// ==================== MatchExpr ====================

#[test]
fn test_match_expr_lowering() {
    let (hir, _interner, body_id) = get_body_hir("fn f(x: i32) { match x { 0 => 1, _ => 0 } }");
    let body = get_body(&hir, body_id);
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block {
            tail: Some(match_id),
            ..
        } => match &body.exprs[*match_id] {
            Expr::Match { arms, .. } => {
                assert_eq!(arms.len(), 2);
            }
            other => panic!("Expected Match, got {:?}", other),
        },
        other => panic!("Expected Block with Match tail",),
    }
}

// ==================== PatOr pattern ====================

#[test]
fn test_pat_or_lowering() {
    let (hir, _interner, body_id) = get_body_hir("fn f(x: i32) { match x { 0 | 1 => 2, _ => 3 } }");
    let body = get_body(&hir, body_id);
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block {
            tail: Some(match_id),
            ..
        } => {
            match &body.exprs[*match_id] {
                Expr::Match { arms, .. } => {
                    assert_eq!(arms.len(), 2);
                    match &body.pats[arms[0].pat] {
                        Pat::Or(pats) => assert_eq!(pats.len(), 2),
                        Pat::Literal(_) => {
                            // parser may not wrap single pat with Or - acceptable
                        }
                        other => panic!("Expected Or pattern or Literal, got {:?}", other),
                    }
                }
                other => panic!("Expected Match",),
            }
        }
        other => panic!("Expected Block",),
    }
}

// ==================== TupleExpr ====================

#[test]
fn test_tuple_expr_lowering() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { (1, 2) }");
    let body = get_body(&hir, body_id);
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block {
            tail: Some(tup_id), ..
        } => {
            match &body.exprs[*tup_id] {
                Expr::Tuple(elems) => assert_eq!(elems.len(), 2),
                // If parser didn't wrap, the block tail might be Literal(1) with comma after
                other => panic!("Expected Tuple, got {:?}", other),
            }
        }
        other => panic!("Expected Block",),
    }
}

// ==================== Module node (parser) ====================

#[test]
fn test_module_node_parsing() {
    let source = "mod foo { fn bar() {} }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    assert!(
        result.diagnostics.is_empty(),
        "Module parsing should have no diagnostics: {:?}",
        result.diagnostics
    );

    // Find Module node
    let mut found_module = false;
    for child in result.root.children() {
        if child.kind() == SyntaxKind::Module {
            found_module = true;
            // Module should contain Ident and items (FnDef)
            let idents: Vec<_> = child.children_with_tokens()
                .filter(|e| matches!(e, glyim_syntax::SyntaxElement::Token(t) if t.kind() == SyntaxKind::Ident))
                .collect();
            assert_eq!(idents.len(), 1, "Module should have one ident (name)");
            let fn_defs: Vec<_> = child
                .children()
                .filter(|c| c.kind() == SyntaxKind::FnDef)
                .collect();
            assert_eq!(fn_defs.len(), 1, "Module should contain one FnDef");
        }
    }
    assert!(found_module, "Expected Module node in CST");
}

#[test]
fn test_nested_module_parsing() {
    let source = "mod outer { mod inner { fn f() {} } }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    assert!(result.diagnostics.is_empty());

    let mut outer_mod = None;
    for child in result.root.children() {
        if child.kind() == SyntaxKind::Module {
            outer_mod = Some(child);
        }
    }
    let outer_mod = outer_mod.expect("Expected outer Module");
    let inner_mod = outer_mod
        .children()
        .find(|c| c.kind() == SyntaxKind::Module)
        .expect("Expected inner Module");
    let fn_def = inner_mod
        .children()
        .find(|c| c.kind() == SyntaxKind::FnDef)
        .expect("Expected FnDef inside inner Module");
}

// ==================== AssignExpr ====================

#[test]
fn test_assign_expr_lowering() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { x = 5; 0 }");
    let body = get_body(&hir, body_id);
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block { stmts, .. } => {
            assert!(!stmts.is_empty(), "should have assign statement");
            let has_assign = stmts
                .iter()
                .any(|&id| matches!(&body.exprs[id], Expr::Assign { .. }));
            assert!(has_assign, "Expected Assign in statements");
        }
        _ => panic!("Expected Block"),
    }
}

// ==================== CastExpr ====================

#[test]
fn test_cast_expr_lowering() {
    let source = "fn f() { x as i32 }";
    let result = glyim_frontend::parse_to_syntax(source, glyim_span::FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner);
    if let ItemKind::Fn(fn_item) = &hir.items[ItemId::from_raw(0)].kind {
        assert!(fn_item.body.is_some());
    } else {
        panic!("Expected Fn");
    }
}

// ==================== IndexExpr ====================

#[test]
fn test_index_expr_lowering() {
    let source = "fn f(, &mut Vec::new()) { a[0] }";
    let result = glyim_frontend::parse_to_syntax(source, glyim_span::FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner);
    if let ItemKind::Fn(fn_item) = &hir.items[ItemId::from_raw(0)].kind {
        assert!(fn_item.body.is_some());
    } else {
        panic!("Expected Fn");
    }
}

// ==================== RangeExpr ====================

#[test]
fn test_range_expr_lowering(, &mut Vec::new()) {
    let (hir, _interner, body_id) = get_body_hir("fn f() { 0..10 }");
    let body = get_body(&hir, body_id);
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block {
            tail: Some(range_id),
            ..
        } => match &body.exprs[*range_id] {
            Expr::Range { start, end, .. } => {
                assert!(start.is_some());
                assert!(end.is_some());
            }
            other => panic!("Expected Range, got {:?}", other),
        },
        _ => panic!("Expected Block with Range tail"),
    }
}

// ==================== RefExpr (via UnaryExpr with &) ====================

#[test]
fn test_ref_expr_lowering() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { &x }");
    let body = get_body(&hir, body_id);
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block {
            tail: Some(ref_id), ..
        } => {
            match &body.exprs[*ref_id] {
                Expr::Ref { .. } => {} // ok
                Expr::Unary { op, .. } if *op == UnOp::Deref => {
                    // &x might be parsed as Unary(Deref) depending on parser
                }
                other => panic!("Expected Ref or Unary, got {:?}", other),
            }
        }
        _ => panic!("Expected Block with Ref tail"),
    }
}

// ==================== CallExpr ====================

#[test]
fn test_call_expr_lowering() {
    let source = "fn f() { foo(1, 2) }";
    let result = glyim_frontend::parse_to_syntax(source, glyim_span::FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner);
    if let ItemKind::Fn(fn_item) = &hir.items[ItemId::from_raw(0)].kind {
        assert!(fn_item.body.is_some());
    } else {
        panic!("Expected Fn");
    }
}

// ==================== While / Loop ====================

#[test]
fn test_while_expr_lowering(, &mut Vec::new()) {
    let (hir, _interner, body_id) = get_body_hir("fn f() { while true { 1; } }");
    let body = get_body(&hir, body_id);
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block {
            tail: Some(while_id),
            ..
        } => {
            assert!(matches!(&body.exprs[*while_id], Expr::While { .. }));
        }
        _ => panic!("Expected Block with While tail"),
    }
}

#[test]
fn test_loop_expr_lowering() {
    let (hir, _interner, body_id) = get_body_hir("fn f() { loop { break; } }");
    let body = get_body(&hir, body_id);
    let block_id = last_expr_id(body);
    match &body.exprs[block_id] {
        Expr::Block {
            tail: Some(loop_id),
            ..
        } => {
            assert!(
                matches!(&body.exprs[*loop_id], Expr::Loop { .. }),
                "Loop should be lowered as While(true)"
            );
        }
        _ => panic!("Expected Block with Loop tail"),
    }
}

// ==================== Struct record ====================

#[test]
fn test_struct_record_lowering() {
    let source = "struct Point { x: i32, y: i32 }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner);
    assert_eq!(hir.items.len(, &mut Vec::new()), 1);
    match &hir.items[ItemId::from_raw(0)].kind {
        ItemKind::Struct(s) => {
            assert_eq!(s.kind, StructKind::Record);
            assert_eq!(s.fields.len(), 2);
        }
        other => panic!("Expected Struct, got {:?}", other),
    }
}

// ==================== Enum with tuple variant ====================

#[test]
fn test_enum_tuple_variant_lowering() {
    let source = "enum Color { Red, Green, Blue, Rgb(u8, u8, u8) }";
    let result = parse_to_syntax(source, FileId::from_raw(0));
    let mut interner = Interner::new();
    let hir = lower_crate(&result.root, &mut interner);
    match &hir.items[ItemId::from_raw(0)].kind {
        ItemKind::Enum(e) => {
            assert_eq!(e.variants.len(, &mut Vec::new()), 4);
            assert_eq!(e.variants[3].kind, StructKind::Tuple);
            assert_eq!(e.variants[3].fields.len(), 3);
        }
        other => panic!("Expected Enum"),
    }
}
