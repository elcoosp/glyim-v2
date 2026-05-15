use crate::pipeline_context::PipelineLowerCtx;
use glyim_core::Visibility;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::AdtId;
use glyim_core::path::PathKind;
use glyim_core::primitives::StructKind;
use glyim_hir::{
    CrateHir, EnumItem, Field as HirField, Item, ItemId, ItemKind, StructItem, Variant,
};
use glyim_lower::{AdtDef, AdtKind, LowerCtx};
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use glyim_test::with_fresh_ty_ctx;

fn dummy_span() -> Span {
    Span::new(
        FileId::BOGUS,
        ByteIdx::ZERO,
        ByteIdx::from_raw(1),
        SyntaxContext::ROOT,
    )
}

fn make_hir_with_struct(
    name: &str,
    field_names: Vec<&str>,
    ctx: &mut glyim_type::TyCtxMut,
) -> (CrateHir, AdtId) {
    use glyim_hir::{Path, PathSegment, TypeRef};

    let interner = ctx.resolver();
    let fields: Vec<HirField> = field_names
        .iter()
        .map(|fname| HirField {
            name: interner.intern(fname),
            ty: TypeRef::Path(Path {
                segments: vec![PathSegment {
                    name: interner.intern("i32"),
                    generic_args: None,
                }],
                kind: PathKind::Plain,
            }),
            span: dummy_span(),
        })
        .collect();

    let item = Item {
        id: ItemId::from_raw(0),
        name: interner.intern(name),
        kind: ItemKind::Struct(StructItem {
            fields,
            kind: StructKind::Record,
            generic_params: vec![],
            where_clauses: vec![],
        }),
        visibility: Visibility::Public,
        span: dummy_span(),
    };

    let mut items: IndexVec<ItemId, Item> = IndexVec::new();
    items.push(item);

    let hir = CrateHir {
        items,
        bodies: IndexVec::new(),
        body_owners: IndexVec::new(),
    };

    (hir, AdtId::from_raw(0))
}

fn make_hir_with_enum(
    name: &str,
    variant_specs: Vec<(&str, Vec<&str>)>,
    ctx: &mut glyim_type::TyCtxMut,
) -> (CrateHir, AdtId) {
    use glyim_hir::{Path, PathSegment, TypeRef};

    let interner = ctx.resolver();
    let variants: Vec<Variant> = variant_specs
        .iter()
        .map(|(vname, field_names)| {
            let fields: Vec<HirField> = field_names
                .iter()
                .map(|fname| HirField {
                    name: interner.intern(fname),
                    ty: TypeRef::Path(Path {
                        segments: vec![PathSegment {
                            name: interner.intern("i32"),
                            generic_args: None,
                        }],
                        kind: PathKind::Plain,
                    }),
                    span: dummy_span(),
                })
                .collect();
            Variant {
                name: interner.intern(vname),
                fields,
                kind: StructKind::Tuple,
                span: dummy_span(),
            }
        })
        .collect();

    let item = Item {
        id: ItemId::from_raw(0),
        name: interner.intern(name),
        kind: ItemKind::Enum(EnumItem {
            variants,
            generic_params: vec![],
            where_clauses: vec![],
        }),
        visibility: Visibility::Public,
        span: dummy_span(),
    };

    let mut items: IndexVec<ItemId, Item> = IndexVec::new();
    items.push(item);

    let hir = CrateHir {
        items,
        bodies: IndexVec::new(),
        body_owners: IndexVec::new(),
    };

    (hir, AdtId::from_raw(0))
}

#[test]
fn test_adt_def_struct_fields() {
    let (frozen_ctx, (hir, adt_id)) =
        with_fresh_ty_ctx(|ctx| make_hir_with_struct("MyStruct", vec!["x", "y", "z"], ctx));

    let lower_ctx = PipelineLowerCtx::new(&frozen_ctx, &hir);
    let def: AdtDef = lower_ctx.adt_def(adt_id);

    assert_eq!(def.variants.len(), 1, "struct should have one variant");
    let variant = &def.variants[0];
    assert_eq!(variant.fields.len(), 3, "should have three fields");
    assert!(matches!(def.kind, AdtKind::Struct), "kind should be Struct");
}

#[test]
fn test_adt_def_enum_variants() {
    let (frozen_ctx, (hir, adt_id)) = with_fresh_ty_ctx(|ctx| {
        make_hir_with_enum(
            "MyEnum",
            vec![("A", vec!["0"]), ("B", vec!["0", "1"]), ("C", vec![])],
            ctx,
        )
    });

    let lower_ctx = PipelineLowerCtx::new(&frozen_ctx, &hir);
    let def: AdtDef = lower_ctx.adt_def(adt_id);

    assert_eq!(def.variants.len(), 3, "enum should have three variants");
    assert_eq!(def.variants[0].fields.len(), 1);
    assert_eq!(def.variants[1].fields.len(), 2);
    assert_eq!(def.variants[2].fields.len(), 0);
    assert!(matches!(def.kind, AdtKind::Enum), "kind should be Enum");
}

#[test]
fn test_lowering_uses_adt_def_for_field_access() {
    let (frozen_ctx, (hir, adt_id)) =
        with_fresh_ty_ctx(|ctx| make_hir_with_struct("Point", vec!["x", "y"], ctx));

    let lower_ctx = PipelineLowerCtx::new(&frozen_ctx, &hir);
    let def: AdtDef = lower_ctx.adt_def(adt_id);

    assert_eq!(def.variants.len(), 1, "should have one variant");
    assert_eq!(def.variants[0].fields.len(), 2, "should have two fields");
    assert!(matches!(def.kind, AdtKind::Struct));
}
