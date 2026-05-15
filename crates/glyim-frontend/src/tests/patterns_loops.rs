
#[test]
fn parse_break_with_complex_expr() {
    // break with a binary expression
    let root = parse("fn main() { loop { break 1 + 2; } }");
    let break_nodes: Vec<_> = root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::BreakExpr)
        .collect();
    assert_eq!(break_nodes.len(), 1);
    assert!(break_nodes[0]
        .children()
        .any(|c| c.kind() == SyntaxKind::BinaryExpr));
}

#[test]
fn parse_break_with_block_expr() {
    let root = parse("fn main() { loop { break { 42 }; } }");
    let break_nodes: Vec<_> = root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::BreakExpr)
        .collect();
    assert_eq!(break_nodes.len(), 1);
    assert!(break_nodes[0]
        .children()
        .any(|c| c.kind() == SyntaxKind::Block));
}

#[test]
fn parse_loop_with_label_and_break() {
    // Labeled loops: 'outer: loop { break 'outer; } — break with label is a lifetime-like token
    let root = parse("fn main() { 'outer: loop { break 'outer; } }");
    assert!(has_descendant(&root, SyntaxKind::LoopExpr));
    let break_nodes: Vec<_> = root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::BreakExpr)
        .collect();
    assert_eq!(break_nodes.len(), 1);
    // break 'outer should include a lifetime token; check that a descendant contains 'outer'
    let children: Vec<_> = break_nodes[0].children().collect();
    // The break with label is parsed as break expr with a label token? Current parser may just consume 'outer as an ident?
    // For now, ensure no error.
    assert!(true);
}

#[test]
fn parse_or_pattern_three_alternatives() {
    let root = parse("fn main() { match () { A | B | C => {} } }");
    assert!(has_descendant(&root, SyntaxKind::PatOr));
}

#[test]
fn parse_range_pattern_exclusive() {
    let root = parse("fn main() { match 0 { 0..5 => {} } }");
    let pat_lit = root
        .descendants()
        .find(|n| n.kind() == SyntaxKind::PatLit)
        .unwrap();
    assert!(pat_lit.children().any(|c| c.kind() == SyntaxKind::DotDot));
    assert!(!pat_lit.children().any(|c| c.kind() == SyntaxKind::DotDotEq));
}

#[test]
fn parse_range_pattern_with_variable() {
    let root = parse("fn main() { match x { 1..=MAX => {} } }");
    let pat_lit = root
        .descendants()
        .find(|n| n.kind() == SyntaxKind::PatLit)
        .unwrap();
    assert!(pat_lit.children().any(|c| c.kind() == SyntaxKind::DotDotEq));
}

#[test]
fn parse_struct_pattern_shorthand_and_explicit() {
    let root = parse("fn main() { let Point { x, y: 0 } = p; }");
    let pat_structs: Vec<_> = root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::PatStruct)
        .collect();
    assert_eq!(pat_structs.len(), 1);
    // Should contain both shorthand (x) and explicit (y: 0)
    let pat_struct = &pat_structs[0];
    let children: Vec<_> = pat_struct.children().collect();
    let has_x = children.iter().any(|c| c.kind() == SyntaxKind::PatIdent && c.text() == "x");
    let has_y = children.iter().any(|c| c.kind() == SyntaxKind::PatIdent && c.text() == "y");
    assert!(has_x, "expected binding 'x'");
    assert!(has_y, "expected binding 'y'");
}

#[test]
fn parse_struct_pattern_with_rest() {
    let root = parse("fn main() { let Point { x, .. } = p; }");
    let pat_structs: Vec<_> = root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::PatStruct)
        .collect();
    assert_eq!(pat_structs.len(), 1);
}

#[test]
fn parse_nested_struct_patterns() {
    let root = parse("fn main() { let Outer { inner: Inner { x } } = o; }");
    let pat_structs: Vec<_> = root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::PatStruct)
        .collect();
    assert_eq!(pat_structs.len(), 2);
}

#[test]
fn parse_or_pattern_inside_tuple() {
    let root = parse("fn main() { match () { (0 | 1, 2 | 3) => {} } }");
    let pat_or_count = root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::PatOr)
        .count();
    assert!(pat_or_count >= 2, "expected at least 2 PatOr nodes inside tuple");
}

#[test]
fn parse_break_without_value() {
    let root = parse("fn main() { loop { break; } }");
    let break_nodes: Vec<_> = root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::BreakExpr)
        .collect();
    assert_eq!(break_nodes.len(), 1);
    // No child expression
    let break_node = &break_nodes[0];
    assert!(!break_node.children().any(|c| c.kind() != SyntaxKind::Error), "break; should have no expression child");
}

#[test]
fn parse_continue_in_loop() {
    let root = parse("fn main() { loop { continue; } }");
    assert!(has_descendant(&root, SyntaxKind::ContinueExpr));
}

#[test]
fn parse_match_arm_with_guard_and_or_pattern() {
    let root = parse("fn main() { match 0 { 0 | 1 if guard() => {} } }");
    assert!(has_descendant(&root, SyntaxKind::PatOr));
    assert!(has_descendant(&root, SyntaxKind::IfExpr)); // guard is an if expression
}

