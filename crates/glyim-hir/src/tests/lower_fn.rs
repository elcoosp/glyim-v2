use crate::lower::lower_crate;
use crate::{ItemId, ItemKind};
use glyim_core::interner::Interner;
use glyim_frontend::parse_to_syntax;
use glyim_span::FileId;

#[test]
fn test_fn_with_params() {
    let source = "fn add(x: i32, y: i32) -> i32 { x + y }";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let hir = lower_crate(&parse_result.root, &mut interner, &mut Vec::new());
    assert_eq!(hir.items.len(), 1);
    let item = &hir.items[ItemId::from_raw(0)];
    assert_eq!(interner.resolve(item.name), "add");
    match &item.kind {
        ItemKind::Fn(fn_item) => {
            assert_eq!(fn_item.params.len(), 2);
            assert_eq!(interner.resolve(fn_item.params[0].name), "x");
            assert_eq!(interner.resolve(fn_item.params[1].name), "y");
            assert!(fn_item.return_ty.is_some());
            assert!(fn_item.body.is_some());
        }
        _ => panic!(),
    }
}

#[test]
fn test_fn_without_body_foreign() {
    let source = "extern \"C\" { fn rand() -> i32; }";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let mut interner = Interner::new();
    let hir = lower_crate(&parse_result.root, &mut interner, &mut Vec::new());
    // Find the function named "rand" and verify it has no body
    let found = hir.items.iter().any(|item| {
        interner.resolve(item.name) == "rand"
            && matches!(&item.kind, ItemKind::Fn(fn_item) if fn_item.body.is_none())
    });
    assert!(found, "Foreign function rand not found or has body");
}
