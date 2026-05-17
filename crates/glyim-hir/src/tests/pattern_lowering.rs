use glyim_core::interner::Interner;
use glyim_syntax::{GreenNode, GreenToken, SyntaxKind, SyntaxNode};
use crate::lower::lower_pat;
use crate::Pat;

fn unknown_pat_node() -> SyntaxNode {
    // GreenNode expects rowan::green::SyntaxKind, but we don't need a real node for this test.
    // Instead, just create a dummy node with Error kind using the correct API.
    // Simpler: use a SyntaxNode from a dummy parse.
    let green = GreenNode::new(glyim_syntax::GlyimLang::kind_to_raw(SyntaxKind::Error), vec![]);
    SyntaxNode::new_root(green)
}

#[test]
fn test_unknown_pattern_returns_err() {
    let node = unknown_pat_node();
    let mut interner = Interner::default();
    let mut pats = glyim_core::arena::IndexVec::new();
    let pat_id = lower_pat(&node, &mut interner, &mut pats);
    assert!(pat_id.is_some());
    match &pats[pat_id.unwrap()] {
        Pat::Err => {}
        _ => panic!("expected Pat::Err for unknown pattern"),
    }
}
