use glyim_core::Visibility;
use glyim_core::arena::IndexVec;
use glyim_core::interner::Interner;
use glyim_hir::{BodyId, CrateHir, FnItem, Item, ItemId, ItemKind, Path, StaticItem, TypeRef};
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use glyim_syntax::{GlyimLang, SyntaxKind, SyntaxNode};
use glyim_type::TyCtxMut;
use rowan::Language;

use crate::discovery::discover_mono_roots;
use crate::mono::MonoItem;

fn make_name(interner: &mut Interner, s: &str) -> glyim_core::interner::Name {
    interner.intern(s)
}

fn make_hir(items: Vec<Item>) -> CrateHir {
    CrateHir {
        items: {
            let mut iv = IndexVec::new();
            for item in items {
                iv.push(item);
            }
            iv
        },
        bodies: IndexVec::new(),
        body_owners: IndexVec::new(),
    }
}

fn dummy_span() -> Span {
    Span::new(
        FileId::BOGUS,
        ByteIdx::ZERO,
        ByteIdx::from_raw(1),
        SyntaxContext::ROOT,
    )
}

fn fn_item(interner: &mut Interner, id: u32, name: &str, body: Option<BodyId>) -> Item {
    Item {
        id: ItemId::from_raw(id),
        name: make_name(interner, name),
        kind: ItemKind::Fn(FnItem {
            params: vec![],
            return_ty: None,
            body,
            is_unsafe: false,
            is_async: false,
            generic_params: vec![],
        }),
        visibility: Visibility::Public,
        span: dummy_span(),
    }
}

fn static_item(interner: &mut Interner, id: u32, name: &str) -> Item {
    Item {
        id: ItemId::from_raw(id),
        name: make_name(interner, name),
        kind: ItemKind::Static(StaticItem {
            ty: TypeRef::Path(Path::from_single(make_name(interner, "i32"))),
            body: None,
            is_mut: false,
        }),
        visibility: Visibility::Public,
        span: dummy_span(),
    }
}

fn empty_syntax_root() -> SyntaxNode {
    let mut builder = rowan::GreenNodeBuilder::new();
    let kind = GlyimLang::kind_to_raw(SyntaxKind::SourceFile);
    builder.start_node(kind);
    builder.finish_node();
    let green = builder.finish();
    SyntaxNode::new_root(green)
}

fn make_ctx(interner: Interner) -> TyCtxMut {
    TyCtxMut::new(interner)
}

#[test]
fn t01_main_function_detected() {
    let mut interner = Interner::default();
    let item = fn_item(&mut interner, 0, "main", None);
    let hir = make_hir(vec![item]);
    let root = empty_syntax_root();
    let mut ctx = make_ctx(interner);
    let (items, diags) = discover_mono_roots(&root, &hir, &mut ctx);
    assert_eq!(items.len(), 1, "expected one root item");
    assert!(matches!(&items[0], MonoItem::Fn { .. }));
    assert!(diags.is_empty(), "expected no diagnostics, got {:?}", diags);
}

#[test]
fn t02_no_entry_points_warning() {
    let mut interner = Interner::default();
    let item = fn_item(&mut interner, 0, "not_main", None);
    let hir = make_hir(vec![item]);
    let root = empty_syntax_root();
    let mut ctx = make_ctx(interner);
    let (items, diags) = discover_mono_roots(&root, &hir, &mut ctx);
    assert!(items.is_empty());
    assert!(!diags.is_empty());
    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("no entry points"));
}

#[test]
fn t03_multiple_entry_points() {
    let mut interner = Interner::default();
    let main_item = fn_item(&mut interner, 0, "main", None);
    let other_item = fn_item(&mut interner, 1, "other", None);
    let hir = make_hir(vec![main_item, other_item]);
    let root = empty_syntax_root();
    let mut ctx = make_ctx(interner);
    let (items, diags) = discover_mono_roots(&root, &hir, &mut ctx);
    assert_eq!(items.len(), 1);
    assert!(diags.is_empty());
}

#[test]
fn t04_static_without_used_not_detected() {
    let mut interner = Interner::default();
    let item = static_item(&mut interner, 0, "MY_STATIC");
    let hir = make_hir(vec![item]);
    let root = empty_syntax_root();
    let mut ctx = make_ctx(interner);
    let (items, diags) = discover_mono_roots(&root, &hir, &mut ctx);
    assert!(items.is_empty());
    assert!(!diags.is_empty());
}

#[test]
fn t05_fn_not_main_not_detected() {
    let mut interner = Interner::default();
    let item = fn_item(&mut interner, 0, "helper", None);
    let hir = make_hir(vec![item]);
    let root = empty_syntax_root();
    let mut ctx = make_ctx(interner);
    let (items, _) = discover_mono_roots(&root, &hir, &mut ctx);
    assert!(items.is_empty());
}
