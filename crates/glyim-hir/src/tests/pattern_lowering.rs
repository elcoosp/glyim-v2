use glyim_core::interner::Interner;
use glyim_span::FileId;
use glyim_frontend::parse_to_syntax;
use glyim_syntax::SyntaxKind;
use crate::lower::lower_pat;
use crate::Pat;

#[test]
fn test_unknown_pattern_returns_err() {
    // Use a completely invalid pattern: a single '&' token.
    let src = "fn main() { let & = 5; }";
    let parse = parse_to_syntax(src, FileId::from_raw(1));
    let fn_def = parse.root.children().find(|n| n.kind() == SyntaxKind::FnDef).expect("FnDef not found");
    let block = fn_def.children().find(|n| n.kind() == SyntaxKind::Block).expect("Block not found");
    let let_stmt = block.children().find(|n| n.kind() == SyntaxKind::LetStmt).expect("LetStmt not found");
    // The parser may produce an Error node for the invalid pattern
    let pat_node = let_stmt.children().find(|n| n.kind() == SyntaxKind::Error)
        .or_else(|| let_stmt.children().find(|_| true)) // fallback to any child that is not Eq or expression
        .expect("No pattern node found");
    let mut interner = Interner::default();
    let mut pats = glyim_core::arena::IndexVec::new();
    let pat_id = lower_pat(&pat_node, &mut interner, &mut pats);
    assert!(pat_id.is_some());
    match &pats[pat_id.unwrap()] {
        Pat::Err => {},
        other => panic!("expected Pat::Err, got {:?}", other),
    }
}
