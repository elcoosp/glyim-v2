use glyim_core::interner::Interner;
use glyim_span::FileId;
use glyim_frontend::parse_to_syntax;
use glyim_syntax::SyntaxKind;
use crate::lower::lower_pat;
use crate::Pat;

fn unknown_pat_node() -> glyim_syntax::SyntaxNode {
    // Create a minimal error node by parsing something that produces an error node
    let parse = parse_to_syntax("!!!", FileId::from_raw(1));
    // The root may contain an error node; we'll just take the first node that is Error kind
    parse.root
        .children()
        .find(|n| n.kind() == SyntaxKind::Error)
        .unwrap_or_else(|| parse.root.clone()) // fallback to root which might be Error
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
