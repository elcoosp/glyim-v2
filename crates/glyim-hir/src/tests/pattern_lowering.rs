use crate::Pat;
use crate::lower::lower_pat;
use glyim_core::interner::Interner;
use glyim_syntax::{GlyimLang, GreenNode, SyntaxKind, SyntaxNode};
use rowan::Language;

#[test]
fn test_unknown_pattern_returns_err() {
    // Create a dummy node with SyntaxKind::Error directly.
    let green = GreenNode::new(GlyimLang::kind_to_raw(SyntaxKind::Error), vec![]);
    let node = SyntaxNode::new_root(green);
    let mut interner = Interner::default();
    let mut pats = glyim_core::arena::IndexVec::new();
    let pat_id = lower_pat(&node, &mut interner, &mut pats);
    assert!(pat_id.is_some());
    match &pats[pat_id.unwrap()] {
        Pat::Err => {}
        other => panic!("expected Pat::Err, got {:?}", other),
    }
}
