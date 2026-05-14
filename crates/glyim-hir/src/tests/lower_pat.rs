use crate::lower::lower_crate;
use crate::{BodyId, ItemId, ItemKind, Pat, PatId};
use glyim_core::interner::Interner;
use glyim_core::primitives::Mutability;
use glyim_frontend::parse_to_syntax;
use glyim_span::FileId;

fn get_body_pat(source: &str) -> (crate::CrateHir, Interner, BodyId) {
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let hir = lower_crate(&parse_result.root, &mut interner);
    let body_id = match &hir.items[ItemId::from_raw(0)].kind {
        ItemKind::Fn(fn_item) => fn_item.body.expect("no body"),
        other => panic!("Expected Fn item, got {:?}", other),
    };
    (hir, interner, body_id)
}

#[test]
fn test_pat_wild() {
    let (hir, _interner, body_id) = get_body_pat("fn f(_: i32) {}");
    let body = &hir.bodies[body_id];
    assert!(!body.pats.is_empty(), "body should have parameter patterns");
    let pat = &body.pats[PatId::from_raw(0)];
    match pat {
        Pat::Wild => {}
        other => panic!("Expected Wild pattern, got {:?}", other),
    }
}

#[test]
fn test_pat_binding() {
    let (hir, interner, body_id) = get_body_pat("fn f(x: i32) {}");
    let body = &hir.bodies[body_id];
    let pat = &body.pats[PatId::from_raw(0)];
    match pat {
        Pat::Binding {
            name,
            mutability,
            subpattern,
        } => {
            assert_eq!(interner.resolve(*name), "x");
            assert_eq!(*mutability, Mutability::Not);
            assert!(subpattern.is_none());
        }
        other => panic!("Expected Binding pattern, got {:?}", other),
    }
}
