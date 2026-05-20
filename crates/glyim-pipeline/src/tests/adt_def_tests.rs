use glyim_core::def_id::AdtId;
use glyim_hir::{CrateHir, Item, ItemId, ItemKind, StructItem, EnumItem, Variant, Field};
use glyim_span::Span;
use glyim_test::test_frozen_ty_ctx;
use crate::pipeline_context::PipelineLowerCtx;

#[test]
fn adt_def_from_hir_struct_with_fields() {
    let ty_ctx = test_frozen_ty_ctx();
    let mut hir = CrateHir::default();
    // Create a struct with two fields
    let struct_item = Item {
        id: ItemId::from_raw(1),
        name: "TestStruct".into(),
        kind: ItemKind::Struct(StructItem {
            fields: vec![
                Field { name: "a".into(), ty: glyim_hir::TypeRef::Path(glyim_hir::Path::from_single("i32".into())), span: Span::DUMMY },
                Field { name: "b".into(), ty: glyim_hir::TypeRef::Path(glyim_hir::Path::from_single("bool".into())), span: Span::DUMMY },
            ],
            kind: glyim_hir::StructKind::Tuple,
            generic_params: vec![],
            where_clauses: vec![],
        }),
        visibility: glyim_core::visibility::Visibility::Public,
        span: Span::DUMMY,
    };
    hir.items.push(struct_item);
    let ctx = PipelineLowerCtx::new(&ty_ctx, &hir);
    let adt_id = AdtId::from_raw(1);
    let adt_def = ctx.adt_def(adt_id);
    assert_eq!(adt_def.variants.len(), 1);
    assert_eq!(adt_def.variants[0].fields.len(), 2);
}

#[test]
fn adt_def_from_hir_enum_with_variants() {
    let ty_ctx = test_frozen_ty_ctx();
    let mut hir = CrateHir::default();
    let enum_item = Item {
        id: ItemId::from_raw(2),
        name: "TestEnum".into(),
        kind: ItemKind::Enum(EnumItem {
            variants: vec![
                Variant { name: "A".into(), fields: vec![], kind: glyim_hir::StructKind::Unit, span: Span::DUMMY },
                Variant { name: "B".into(), fields: vec![Field { name: "x".into(), ty: glyim_hir::TypeRef::Path(glyim_hir::Path::from_single("u32".into())), span: Span::DUMMY }], kind: glyim_hir::StructKind::Tuple, span: Span::DUMMY },
            ],
            generic_params: vec![],
            where_clauses: vec![],
        }),
        visibility: glyim_core::visibility::Visibility::Public,
        span: Span::DUMMY,
    };
    hir.items.push(enum_item);
    let ctx = PipelineLowerCtx::new(&ty_ctx, &hir);
    let adt_id = AdtId::from_raw(2);
    let adt_def = ctx.adt_def(adt_id);
    assert_eq!(adt_def.variants.len(), 2);
    assert_eq!(adt_def.variants[0].fields.len(), 0);
    assert_eq!(adt_def.variants[1].fields.len(), 1);
}
