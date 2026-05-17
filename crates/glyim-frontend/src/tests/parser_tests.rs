use glyim_span::FileId;
use glyim_syntax::{SyntaxKind, SyntaxNode};
use crate::parse_to_syntax;

fn print_tree(node: &SyntaxNode, indent: usize) {
    let indent_str = "  ".repeat(indent);
    eprintln!("{}{:?} {:?}", indent_str, node.kind(), node.text());
    for child in node.children() {
        print_tree(&child, indent + 1);
    }
}

#[test]
fn test_parse_closure_no_params() {
    let src = "fn main() { || 42; }";
    let parse = parse_to_syntax(src, FileId::from_raw(1));
    eprintln!("=== CST for '{}' ===", src);
    print_tree(&parse.root, 0);
    // Find the ClosureExpr manually
    let fn_def = parse.root.children().find(|n| n.kind() == SyntaxKind::FnDef)
        .expect("FnDef not found");
    let block = fn_def.children().find(|n| n.kind() == SyntaxKind::Block)
        .expect("Block not found");
    eprintln!("Block children:");
    for child in block.children() {
        eprintln!("  {:?}", child.kind());
    }
    // Check if there's an ExprStmt containing the closure
    let expr_stmt = block.children().find(|n| n.kind() == SyntaxKind::ExprStmt);
    if let Some(stmt) = expr_stmt {
        eprintln!("ExprStmt children:");
        for child in stmt.children() {
            eprintln!("    {:?}", child.kind());
        }
    }
    let closure = block.children().find(|n| n.kind() == SyntaxKind::ClosureExpr)
        .or_else(|| {
            block.children()
                .find(|n| n.kind() == SyntaxKind::ExprStmt)
                .and_then(|stmt| stmt.children().find(|c| c.kind() == SyntaxKind::ClosureExpr))
        })
        .expect("ClosureExpr not found");
    let param_list = closure.children().find(|n| n.kind() == SyntaxKind::ParamList)
        .expect("ClosureExpr should contain a ParamList node");
    let params: Vec<_> = param_list.children().collect();
    assert!(params.is_empty(), "ParamList should be empty for no parameters");
}

#[test]
fn test_parse_closure_with_paramlist() {
    let src = "fn main() { |x: i32| x + 1; }";
    let parse = parse_to_syntax(src, FileId::from_raw(1));
    eprintln!("=== CST for '{}' ===", src);
    print_tree(&parse.root, 0);
    let closure = parse.root.children()
        .find(|n| n.kind() == SyntaxKind::FnDef)
        .and_then(|fn_def| fn_def.children().find(|n| n.kind() == SyntaxKind::Block))
        .and_then(|block| block.children().find(|n| n.kind() == SyntaxKind::ClosureExpr))
        .or_else(|| {
            parse.root.children()
                .find(|n| n.kind() == SyntaxKind::FnDef)
                .and_then(|fn_def| fn_def.children().find(|n| n.kind() == SyntaxKind::Block))
                .and_then(|block| block.children().find(|n| n.kind() == SyntaxKind::ExprStmt))
                .and_then(|stmt| stmt.children().find(|c| c.kind() == SyntaxKind::ClosureExpr))
        })
        .expect("ClosureExpr not found");
    let param_list = closure.children().find(|n| n.kind() == SyntaxKind::ParamList)
        .expect("ClosureExpr should contain a ParamList node");
    let params: Vec<_> = param_list.children().filter(|c| c.kind() == SyntaxKind::Param).collect();
    assert_eq!(params.len(), 1, "Should have exactly one Param");
    let param = &params[0];
    let pat = param.children().find(|n| n.kind() == SyntaxKind::PatIdent).unwrap();
    let ty = param.children().find(|n| n.kind() == SyntaxKind::PathType).unwrap();
    let pat_text = pat.text().to_string();
    assert!(pat_text.contains('x'), "Parameter pattern should contain 'x'");
    let ty_text = ty.text().to_string();
    assert!(ty_text.contains("i32"), "Parameter type should contain 'i32'");
}

#[test]
fn test_parse_struct_expr_shorthand_fields() {
    let src = "fn main() { let p = Point { x, y: 20 }; }";
    let parse = parse_to_syntax(src, FileId::from_raw(1));
    eprintln!("=== CST for '{}' ===", src);
    print_tree(&parse.root, 0);
    let let_stmt = parse.root.children()
        .find(|n| n.kind() == SyntaxKind::FnDef)
        .and_then(|fn_def| fn_def.children().find(|n| n.kind() == SyntaxKind::Block))
        .and_then(|block| block.children().find(|n| n.kind() == SyntaxKind::LetStmt))
        .expect("LetStmt not found");
    let struct_expr = let_stmt.children().find(|n| n.kind() == SyntaxKind::StructExpr)
        .expect("StructExpr not found");
    eprintln!("StructExpr children:");
    for child in struct_expr.children() {
        eprintln!("  {:?}", child.kind());
    }
    let fields: Vec<_> = struct_expr.children()
        .filter(|n| n.kind() == SyntaxKind::StructField)
        .collect();
    assert_eq!(fields.len(), 2, "Should have two StructField nodes");
    // Check shorthand field (first field)
    let shorthand_field = &fields[0];
    eprintln!("Shorthand field children:");
    for child in shorthand_field.children() {
        eprintln!("  {:?}", child.kind());
    }
    let path_expr = shorthand_field.children()
        .find(|c| c.kind() == SyntaxKind::PathExpr)
        .expect("Shorthand field should contain a PathExpr");
    // Check that the shorthand field contains a PathExpr (we assume it works)
    assert!(shorthand_field.children().any(|c| c.kind() == SyntaxKind::PathExpr), "Shorthand field missing PathExpr");
    let explicit_field = &fields[1];
    let explicit_name = explicit_field.children()
        .find(|c| c.kind() == SyntaxKind::Ident)
        .expect("Explicit field should have an Ident");
    assert_eq!(explicit_name.text(), "y", "Explicit field name should be 'y'");
    // Also check struct path
    let path = struct_expr.children().find(|n| n.kind() == SyntaxKind::PathExpr).unwrap();
    let path_text = path.text().to_string();
    assert!(path_text.contains("Point"), "Struct path should be 'Point'");
}
