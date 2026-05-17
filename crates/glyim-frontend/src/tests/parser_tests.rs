use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use glyim_syntax::{SyntaxKind, SyntaxNode};
use glyim_frontend::parse_to_syntax;

fn parse_to_node(src: &str, expected_kind: SyntaxKind) -> SyntaxNode {
    let parse = parse_to_syntax(src, FileId::from_raw(1));
    parse.root
        .children()
        .find(|n| n.kind() == expected_kind)
        .unwrap_or_else(|| panic!("Expected node kind {:?} not found in {}", expected_kind, src))
        .clone()
}

#[test]
fn test_parse_closure_with_paramlist() {
    let src = "fn main() { |x: i32| x + 1; }";
    let fn_def = parse_to_node(src, SyntaxKind::FnDef);
    let block = fn_def.children().find(|n| n.kind() == SyntaxKind::Block).unwrap();
    let closure = block.children().find(|n| n.kind() == SyntaxKind::ClosureExpr).unwrap();

    // Verify ParamList exists inside ClosureExpr
    let param_list = closure.children().find(|n| n.kind() == SyntaxKind::ParamList);
    assert!(param_list.is_some(), "ClosureExpr should contain a ParamList node");
    let params: Vec<_> = param_list.unwrap().children().filter(|c| c.kind() == SyntaxKind::Param).collect();
    assert_eq!(params.len(), 1, "Should have exactly one Param");

    // Verify Param contains a pattern and type
    let param = &params[0];
    let pat = param.children().find(|n| n.kind() == SyntaxKind::PatIdent).unwrap();
    let ty = param.children().find(|n| n.kind() == SyntaxKind::PathType).unwrap();
    assert!(pat.children_with_tokens().any(|el| el.into_token().is_some_and(|t| t.text() == "x")));
    assert!(ty.text().contains("i32"));
}

#[test]
fn test_parse_closure_no_params() {
    let src = "fn main() { || 42; }";
    let fn_def = parse_to_node(src, SyntaxKind::FnDef);
    let block = fn_def.children().find(|n| n.kind() == SyntaxKind::Block).unwrap();
    let closure = block.children().find(|n| n.kind() == SyntaxKind::ClosureExpr).unwrap();

    let param_list = closure.children().find(|n| n.kind() == SyntaxKind::ParamList).unwrap();
    let params: Vec<_> = param_list.children().collect();
    assert!(params.is_empty(), "ParamList should be empty for no parameters");
}

#[test]
fn test_parse_closure_with_multiple_params() {
    let src = "fn main() { |a, b: u8| a + b; }";
    let fn_def = parse_to_node(src, SyntaxKind::FnDef);
    let block = fn_def.children().find(|n| n.kind() == SyntaxKind::Block).unwrap();
    let closure = block.children().find(|n| n.kind() == SyntaxKind::ClosureExpr).unwrap();

    let param_list = closure.children().find(|n| n.kind() == SyntaxKind::ParamList).unwrap();
    let params: Vec<_> = param_list.children().filter(|c| c.kind() == SyntaxKind::Param).collect();
    assert_eq!(params.len(), 2, "Should have two parameters");
}

#[test]
fn test_parse_struct_expr_shorthand_fields() {
    let src = "fn main() { let p = Point { x, y: 20 }; }";
    let fn_def = parse_to_node(src, SyntaxKind::FnDef);
    let block = fn_def.children().find(|n| n.kind() == SyntaxKind::Block).unwrap();
    let let_stmt = block.children().find(|n| n.kind() == SyntaxKind::LetStmt).unwrap();
    let struct_expr = let_stmt.children().find(|n| n.kind() == SyntaxKind::StructExpr).unwrap();

    // Find fields inside braces
    let brace_contents: Vec<_> = struct_expr.children().collect();
    let fields: Vec<_> = brace_contents.iter().filter(|n| n.kind() == SyntaxKind::StructField).collect();
    assert_eq!(fields.len(), 2, "Should have two StructField nodes");

    // First field shorthand: "x" should have no colon child, but parser still produces StructField with an Ident? Actually shorthand is a PathExpr.
    // Better to check that there are two field children regardless.
    let path = struct_expr.children().find(|n| n.kind() == SyntaxKind::PathExpr).unwrap();
    assert!(path.text().contains("Point"));
}

#[test]
fn test_parse_closure_with_ret_type() {
    let src = "fn main() { |x| -> i32 { x + 1 } }";
    let fn_def = parse_to_node(src, SyntaxKind::FnDef);
    let block = fn_def.children().find(|n| n.kind() == SyntaxKind::Block).unwrap();
    let closure = block.children().find(|n| n.kind() == SyntaxKind::ClosureExpr).unwrap();

    // After |x| there should be an Arrow token and a type
    let arrow = closure.children_with_tokens().any(|el| {
        el.into_token().is_some_and(|t| t.kind() == SyntaxKind::Arrow)
    });
    assert!(arrow, "Closure should have arrow and return type");
}
