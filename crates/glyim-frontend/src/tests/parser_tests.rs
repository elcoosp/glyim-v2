use crate::lexer::lex;
use crate::parser::{Parser, parse_to_syntax};
use glyim_span::FileId;
use glyim_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

fn file_id() -> FileId {
    FileId::from_raw(1)
}

/// Find the first child element (node or token) of a given kind.
fn first_child_of_kind(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxElement> {
    node.children_with_tokens().find(|c| c.kind() == kind)
}

/// Assert that a child element exists and return it.
fn assert_child_el(node: &SyntaxNode, kind: SyntaxKind) -> SyntaxElement {
    first_child_of_kind(node, kind).unwrap_or_else(|| panic!("missing child {:?}", kind))
}

/// Extract text of a SyntaxElement (works for both nodes and tokens).
fn text_of(el: &SyntaxElement) -> String {
    match el {
        SyntaxElement::Node(n) => n.text().to_string(),
        SyntaxElement::Token(t) => t.text().to_string(),
    }
}

/// Assert that there is a child node of the given kind and return it.
fn assert_child_node(node: &SyntaxNode, kind: SyntaxKind) -> SyntaxNode {
    node.children()
        .find(|c| c.kind() == kind)
        .unwrap_or_else(|| panic!("missing node {:?}", kind))
}

// S09-T01: Parse fn item
#[test]
fn test_parse_fn_def() {
    let result = parse_to_syntax("fn foo() {}", file_id());
    let root = &result.root;
    let fn_def = assert_child_node(root, SyntaxKind::FnDef);
    // The Ident token should be a direct child of FnDef
    let ident_el = assert_child_el(&fn_def, SyntaxKind::Ident);
    assert_eq!(text_of(&ident_el), "foo");
    // ParamList is a node
    assert_child_node(&fn_def, SyntaxKind::ParamList);
    // Block is a node
    assert_child_node(&fn_def, SyntaxKind::Block);
}

// S09-T02: Parse struct unit
#[test]
fn test_parse_struct_unit() {
    let result = parse_to_syntax("struct Foo;", file_id());
    let struct_def = assert_child_node(&result.root, SyntaxKind::StructDef);
    let ident_el = assert_child_el(&struct_def, SyntaxKind::Ident);
    assert_eq!(text_of(&ident_el), "Foo");
}

#[test]
fn test_parse_struct_tuple() {
    let result = parse_to_syntax("struct Pair(i32, f64);", file_id());
    let struct_def = assert_child_node(&result.root, SyntaxKind::StructDef);
    assert_eq!(
        text_of(&assert_child_el(&struct_def, SyntaxKind::Ident)),
        "Pair"
    );
    assert!(result.diagnostics.is_empty(), "should parse without errors");
}

#[test]
fn test_parse_struct_record() {
    let result = parse_to_syntax("struct Rect { x: f64, y: f64 }", file_id());
    let struct_def = assert_child_node(&result.root, SyntaxKind::StructDef);
    assert_child_el(&struct_def, SyntaxKind::Ident);
    assert!(result.diagnostics.is_empty(), "should parse without errors");
}

// S09-T03: Parse enum
#[test]
fn test_parse_enum() {
    let result = parse_to_syntax("enum Color { Red, Green, Blue }", file_id());
    let enum_def = assert_child_node(&result.root, SyntaxKind::EnumDef);
    // Verify variant list exists and contains three variants
    let variant_list = assert_child_node(&enum_def, SyntaxKind::VariantList);
    let variants: Vec<_> = variant_list
        .children()
        .filter(|c| c.kind() == SyntaxKind::EnumVariant)
        .collect();
    assert_eq!(variants.len(), 3);
    // First variant's name is "Red"
    assert_eq!(
        text_of(&assert_child_el(&variants[0], SyntaxKind::Ident)),
        "Red"
    );
}

// S09-T04: Parse trait def
#[test]
fn test_parse_trait_def() {
    let result = parse_to_syntax("trait Draw { fn draw(&self); }", file_id());
    assert_child_node(&result.root, SyntaxKind::TraitDef);
    assert!(
        result.diagnostics.is_empty(),
        "should parse trait without errors"
    );
}

// S09-T05: Parse impl def
#[test]
fn test_parse_impl_def() {
    let result = parse_to_syntax("impl Draw for Circle { fn draw(&self) {} }", file_id());
    assert_child_node(&result.root, SyntaxKind::ImplDef);
    assert!(
        result.diagnostics.is_empty(),
        "should parse impl without errors"
    );
}

// S09-T06: Expression precedence
#[test]
fn test_expr_precedence() {
    let result = parse_to_syntax("fn f() { 1 + 2 * 3; }", file_id());
    let fn_def = assert_child_node(&result.root, SyntaxKind::FnDef);
    let block = assert_child_node(&fn_def, SyntaxKind::Block);
    let expr_stmt = assert_child_node(&block, SyntaxKind::ExprStmt);
    // The top-level binary expression should be '+'
    let bin_expr = assert_child_node(&expr_stmt, SyntaxKind::BinaryExpr);
    let _plus_token = bin_expr
        .children_with_tokens()
        .find_map(|e| {
            if e.kind() == SyntaxKind::Plus {
                Some(e)
            } else {
                None
            }
        })
        .expect("should find '+' operator");
    // Right operand should itself be a BinaryExpr (the '*')
    let right = bin_expr
        .children()
        .find(|c| c.kind() == SyntaxKind::BinaryExpr);
    assert!(right.is_some(), "right operand should be multiplication");
}

// S09-T07: Method calls and field access
#[test]
fn test_method_call() {
    let result = parse_to_syntax("fn f() { a.b(); }", file_id());
    assert!(result.diagnostics.is_empty());
}

#[test]
fn test_field_access() {
    let result = parse_to_syntax("fn f() { a.b; }", file_id());
    assert!(result.diagnostics.is_empty());
    let fn_def = assert_child_node(&result.root, SyntaxKind::FnDef);
    let block = assert_child_node(&fn_def, SyntaxKind::Block);
    let expr_stmt = assert_child_node(&block, SyntaxKind::ExprStmt);
    // Field access is wrapped in FieldExpr with PathExpr as receiver
    let field_expr = assert_child_node(&expr_stmt, SyntaxKind::FieldExpr);
    // Check that it contains a PathExpr (the receiver)
    let _path = assert_child_node(&field_expr, SyntaxKind::PathExpr);
}

// S09-T08: Pattern grammar
#[test]
fn test_pattern_wildcard() {
    let result = parse_to_syntax("fn f() { let _ = 1; }", file_id());
    assert!(result.diagnostics.is_empty());
    let fn_def = assert_child_node(&result.root, SyntaxKind::FnDef);
    let block = assert_child_node(&fn_def, SyntaxKind::Block);
    let let_stmt = assert_child_node(&block, SyntaxKind::LetStmt);
    assert_child_node(&let_stmt, SyntaxKind::PatWild);
}

#[test]
fn test_pattern_ident() {
    let result = parse_to_syntax("fn f() { let x = 1; }", file_id());
    assert!(result.diagnostics.is_empty());
    let fn_def = assert_child_node(&result.root, SyntaxKind::FnDef);
    let block = assert_child_node(&fn_def, SyntaxKind::Block);
    let let_stmt = assert_child_node(&block, SyntaxKind::LetStmt);
    assert_child_node(&let_stmt, SyntaxKind::PatIdent);
}

#[test]
fn test_pattern_ref() {
    let result = parse_to_syntax("fn f() { let ref x = y; }", file_id());
    assert!(result.diagnostics.is_empty());
    let fn_def = assert_child_node(&result.root, SyntaxKind::FnDef);
    let block = assert_child_node(&fn_def, SyntaxKind::Block);
    let let_stmt = assert_child_node(&block, SyntaxKind::LetStmt);
    // After `ref` keyword we still expect a PatIdent
    assert_child_node(&let_stmt, SyntaxKind::PatIdent);
}

#[test]
fn test_pattern_tuple() {
    let result = parse_to_syntax("fn f() { let (a, b) = (1, 2); }", file_id());
    assert!(result.diagnostics.is_empty());
    let fn_def = assert_child_node(&result.root, SyntaxKind::FnDef);
    let block = assert_child_node(&fn_def, SyntaxKind::Block);
    let let_stmt = assert_child_node(&block, SyntaxKind::LetStmt);
    let pat_tuple = assert_child_node(&let_stmt, SyntaxKind::PatTuple);
    let patterns: Vec<_> = pat_tuple
        .children()
        .filter(|c| c.kind() == SyntaxKind::PatIdent)
        .collect();
    assert_eq!(patterns.len(), 2);
}

// S09-T09: Type grammar
#[test]
fn test_type_simple() {
    let result = parse_to_syntax("fn f(x: i32) {}", file_id());
    assert!(result.diagnostics.is_empty());
}

#[test]
fn test_type_ref() {
    let result = parse_to_syntax("fn f(x: &i32) {}", file_id());
    assert!(result.diagnostics.is_empty());
}

#[test]
fn test_type_ref_mut() {
    let result = parse_to_syntax("fn f(x: &mut i32) {}", file_id());
    assert!(result.diagnostics.is_empty());
}

// S09-T10: Error recovery missing semicolon
#[test]
fn test_error_missing_semicolon() {
    let result = parse_to_syntax("fn f() { let x = 5 }", file_id());
    assert!(
        !result.diagnostics.is_empty(),
        "should produce errors for missing semicolon"
    );
    let fn_def = assert_child_node(&result.root, SyntaxKind::FnDef);
    let block = assert_child_node(&fn_def, SyntaxKind::Block);
    assert_child_node(&block, SyntaxKind::LetStmt);
}

// S09-T11: Error recovery mismatched braces
#[test]
fn test_error_mismatched_braces() {
    let result = parse_to_syntax("fn f() { if true { }", file_id());
    assert!(
        !result.diagnostics.is_empty(),
        "should produce errors for mismatched braces"
    );
}

// S09-T12: No token loss
#[test]
fn test_no_token_loss() {
    let source = "fn main() { let x: i32 = 42; x + 1 }";
    let lex_result = lex(source, file_id());
    let tokens = &lex_result.tokens;
    let mut parser = Parser::new(tokens);
    parser.parse_source_file();
    assert_eq!(parser.pos(), tokens.len(), "all tokens should be consumed");
}

// S09-T13: Snapshot CST
#[test]
fn test_snapshot_cst() {
    glyim_test::snapshot_cst("test_add_fn", "fn add(a: i32, b: i32) -> i32 { a + b }");
}

// ----- Helper functions for finding nodes (added by Stream U01) -----

fn find_node_by_kind(root: &SyntaxNode, target_kind: SyntaxKind) -> Option<SyntaxNode> {
    let mut stack = vec![root.clone()];
    while let Some(node) = stack.pop() {
        if node.kind() == target_kind {
            return Some(node);
        }
        stack.extend(node.children());
    }
    None
}

fn parse_to_node_recursive(src: &str, expected_kind: SyntaxKind) -> SyntaxNode {
    let parse = parse_to_syntax(src, FileId::from_raw(1));
    find_node_by_kind(&parse.root, expected_kind).unwrap_or_else(|| {
        panic!(
            "Expected node kind {:?} not found in {}",
            expected_kind, src
        )
    })
}

// ----- Additional tests for closure and struct expressions (added by Stream U01) -----

#[test]
fn test_parse_closure_with_paramlist() {
    let src = "fn main() { |x: i32| x + 1; }";
    let closure = parse_to_node_recursive(src, SyntaxKind::ClosureExpr);
    let param_list = closure
        .children()
        .find(|n| n.kind() == SyntaxKind::ParamList)
        .expect("ClosureExpr should contain a ParamList node");
    let params: Vec<_> = param_list
        .children()
        .filter(|c| c.kind() == SyntaxKind::Param)
        .collect();
    assert_eq!(params.len(), 1, "Should have exactly one Param");
    let param = &params[0];
    let pat = param
        .children()
        .find(|n| n.kind() == SyntaxKind::PatIdent)
        .unwrap();
    let ty = param
        .children()
        .find(|n| n.kind() == SyntaxKind::PathType)
        .unwrap();
    let pat_text = pat.text().to_string();
    assert!(pat_text.contains("x"), "Parameter pattern should contain x");
    let ty_text = ty.text().to_string();
    assert!(ty_text.contains("i32"), "Parameter type should contain i32");
}

#[test]
#[ignore = "Empty closure parsing not yet supported (|| is ambiguous with logical OR)"]
fn test_parse_closure_no_params() {}

#[test]
fn test_parse_closure_with_multiple_params() {
    let src = "fn main() { |a, b: u8| a + b; }";
    let closure = parse_to_node_recursive(src, SyntaxKind::ClosureExpr);
    let param_list = closure
        .children()
        .find(|n| n.kind() == SyntaxKind::ParamList)
        .unwrap();
    let params: Vec<_> = param_list
        .children()
        .filter(|c| c.kind() == SyntaxKind::Param)
        .collect();
    assert_eq!(params.len(), 2, "Should have two parameters");
}

#[test]
fn test_parse_closure_with_ret_type() {
    let src = "fn main() { |x| -> i32 { x + 1 } }";
    let closure = parse_to_node_recursive(src, SyntaxKind::ClosureExpr);
    let has_arrow = closure.children_with_tokens().any(|el| {
        el.into_token()
            .is_some_and(|t| t.kind() == SyntaxKind::Arrow)
    });
    assert!(has_arrow, "Closure should have arrow and return type");
}

#[test]
fn test_parse_struct_expr_shorthand_fields() {
    let src = "fn main() { let p = Point { x, y: 20 }; }";
    let let_stmt = parse_to_node_recursive(src, SyntaxKind::LetStmt);
    let struct_expr = let_stmt
        .children()
        .find(|n| n.kind() == SyntaxKind::StructExpr)
        .expect("StructExpr not found");
    let fields: Vec<_> = struct_expr
        .children()
        .filter(|n| n.kind() == SyntaxKind::StructField)
        .collect();
    assert_eq!(fields.len(), 2, "Should have two StructField nodes");
    let shorthand = &fields[0];
    assert!(
        shorthand
            .children()
            .any(|c| c.kind() == SyntaxKind::PathExpr),
        "Shorthand field missing PathExpr"
    );
}
