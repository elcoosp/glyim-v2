use crate::lower::lower_crate;
use crate::{ItemId, ItemKind};
use glyim_core::interner::Interner;
use glyim_core::primitives::StructKind;
use glyim_frontend::parse_to_syntax;
use glyim_span::FileId;

#[test]
fn test_enum_with_variants() {
    let source = "enum Color { Red, Green, Blue, Rgb(u8, u8, u8) }";
    let file_id = FileId::from_raw(0);
    let parse_result = parse_to_syntax(source, file_id);
    let root = parse_result.root;
    let mut interner = Interner::new();
    let hir = lower_crate(&root, &mut interner, &mut Vec::new());

    assert_eq!(hir.items.len(), 1);
    let item = &hir.items[ItemId::from_raw(0)];
    assert_eq!(interner.resolve(item.name), "Color");
    match &item.kind {
        ItemKind::Enum(e) => {
            assert_eq!(e.variants.len(), 4);
            assert_eq!(interner.resolve(e.variants[0].name), "Red");
            assert_eq!(e.variants[0].kind, StructKind::Unit);
            assert_eq!(interner.resolve(e.variants[3].name), "Rgb");
            assert_eq!(e.variants[3].kind, StructKind::Tuple);
            assert_eq!(e.variants[3].fields.len(), 3);
        }
        other => panic!("Expected Enum item, got {:?}", other),
    }
}
