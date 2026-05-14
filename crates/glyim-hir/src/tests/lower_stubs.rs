use crate::lower::lower_crate;
use glyim_core::interner::Interner;
use glyim_frontend::parse_to_syntax;
use glyim_span::FileId;

/// Test that an unknown expression kind (e.g., WhileExpr) doesn't crash.
#[test]
fn test_while_expr_stub() {
    let source = "fn f() { while true { } }";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let _hir = lower_crate(&parse_result.root, &mut interner);
    // Should not panic; stub warning will be emitted
}

/// Test that a loop expression produces stub.
#[test]
fn test_loop_expr_stub() {
    let source = "fn f() { loop { break; } }";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let _hir = lower_crate(&parse_result.root, &mut interner);
}

/// Test that tuple struct lowering doesn't crash (stub).
#[test]
fn test_tuple_struct_stub() {
    let source = "struct Pair(i32, f64);";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let _hir = lower_crate(&parse_result.root, &mut interner);
    // Should succeed with a stub warning
}

/// Test that float literal doesn't crash.
#[test]
fn test_float_literal_stub() {
    let source = "fn f() { 3.14 }";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let _hir = lower_crate(&parse_result.root, &mut interner);
}

/// Test that an unhandled type node (e.g., DynType) doesn't crash.
#[test]
fn test_dyn_type_stub() {
    let source = "fn f(x: &dyn Display) {}";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let _hir = lower_crate(&parse_result.root, &mut interner);
}

/// Test that match expression stub doesn't crash.
#[test]
fn test_match_expr_stub() {
    let source = "fn f(x: i32) { match x { 0 => 1, _ => 0 } }";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let _hir = lower_crate(&parse_result.root, &mut interner);
}

/// Test that let statement is handled (it's not an expression, but appears inside block).
#[test]
fn test_let_stmt_in_block() {
    let source = "fn f() { let x = 1; x }";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let _hir = lower_crate(&parse_result.root, &mut interner);
}
