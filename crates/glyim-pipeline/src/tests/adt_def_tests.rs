use glyim_core::def_id::AdtId;
use glyim_core::primitives::IntTy;
use glyim_core::primitives::StructKind;
use glyim_core::path::PathKind;
use glyim_core::IndexVec;
use glyim_hir::{CrateHir, Item, ItemKind, StructItem, Field as HirField, Variant, EnumItem};
use glyim_lower::{AdtDef, AdtKind, LowerCtx};
use glyim_test::{with_fresh_ty_ctx, assert_ty};
use crate::pipeline_context::PipelineLowerCtx;

/// Helper: create a minimal HIR with a single struct item.
/// Returns (CrateHir, AdtId) where AdtId uses the item's raw index.
fn make_hir_with_struct(
    name: &str,
    field_names: Vec<&str>,
    ctx: &mut glyim_type::TyCtxMut,
) -> (CrateHir, AdtId) {
    use glyim_hir::{TypeRef, Path, PathSegment};

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
            span: glyim_span::Span::DUMMY,
        })
        .collect();

    let item = Item {
        id: glyim_hir::ItemId::from_raw(0),
        name: interner.intern(name),
        kind: ItemKind::Struct(StructItem {
            fields,
            kind: StructKind::Record,
            generic_params: vec![],
        }),
        visibility: glyim_core::Visibility::Public,
        span: glyim_span::Span::DUMMY,
    };

    let mut items: IndexVec<glyim_hir::ItemId, Item> = IndexVec::new();
    items.push(item);

    let hir = CrateHir {
        items,
        bodies: IndexVec::new(),
        body_owners: IndexVec::new(),
    };

    (hir, AdtId::from_raw(0))
}

/// Helper: create a minimal HIR with a single enum item containing the given variants.
fn make_hir_with_enum(
    name: &str,
    variant_specs: Vec<(&str, Vec<&str>)>,
    ctx: &mut glyim_type::TyCtxMut,
) -> (CrateHir, AdtId) {
    use glyim_hir::{TypeRef, Path, PathSegment};

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
                    span: glyim_span::Span::DUMMY,
                })
                .collect();
            Variant {
                name: interner.intern(vname),
                fields,
                kind: StructKind::Tuple,
                span: glyim_span::Span::DUMMY,
            }
        })
        .collect();

    let item = Item {
        id: glyim_hir::ItemId::from_raw(0),
        name: interner.intern(name),
        kind: ItemKind::Enum(EnumItem {
            variants,
            generic_params: vec![],
        }),
        visibility: glyim_core::Visibility::Public,
        span: glyim_span::Span::DUMMY,
    };

    let mut items: IndexVec<glyim_hir::ItemId, Item> = IndexVec::new();
    items.push(item);

    let hir = CrateHir {
        items,
        bodies: IndexVec::new(),
        body_owners: IndexVec::new(),
    };

    (hir, AdtId::from_raw(0))
}

// U05-T01: PipelineLowerCtx returns correct field types for struct
#[test]
fn test_adt_def_struct_fields() {
    let (frozen_ctx, (hir, adt_id)) = with_fresh_ty_ctx(|ctx| {
        make_hir_with_struct("MyStruct", vec!["x", "y", "z"], ctx)
    });

    let lower_ctx = PipelineLowerCtx::new(&frozen_ctx, &hir);
    let def: AdtDef = lower_ctx.adt_def(adt_id);

    assert_eq!(def.variants.len(), 1, "struct should have one variant");
    let variant = &def.variants[0];
    assert_eq!(variant.fields.len(), 3, "should have three fields");

    // All fields are typed i32 per make_hir_with_struct
    for field_ty in &variant.fields {
        assert_ty(&frozen_ctx, *field_ty).is_int(IntTy::I32);
    }

    assert!(matches!(def.kind, AdtKind::Struct), "kind should be Struct");
}

// U05-T02: PipelineLowerCtx returns variants for enum
#[test]
fn test_adt_def_enum_variants() {
    let (frozen_ctx, (hir, adt_id)) = with_fresh_ty_ctx(|ctx| {
        make_hir_with_enum(
            "MyEnum",
            vec![
                ("A", vec!["0"]),
                ("B", vec!["0", "1"]),
                ("C", vec![]),
            ],
            ctx,
        )
    });

    let lower_ctx = PipelineLowerCtx::new(&frozen_ctx, &hir);
    let def: AdtDef = lower_ctx.adt_def(adt_id);

    assert_eq!(def.variants.len(), 3, "enum should have three variants");

    // Variant A: 1 field
    assert_eq!(def.variants[0].fields.len(), 1);
    assert_ty(&frozen_ctx, def.variants[0].fields[0]).is_int(IntTy::I32);

    // Variant B: 2 fields
    assert_eq!(def.variants[1].fields.len(), 2);
    assert_ty(&frozen_ctx, def.variants[1].fields[0]).is_int(IntTy::I32);
    assert_ty(&frozen_ctx, def.variants[1].fields[1]).is_int(IntTy::I32);

    // Variant C: 0 fields
    assert_eq!(def.variants[2].fields.len(), 0);

    assert!(matches!(def.kind, AdtKind::Enum), "kind should be Enum");
}

// U05-T03: Lowering uses real ADT def for field access
// This test verifies that the PipelineLowerCtx produces an AdtDef whose field
// types are reflected as real i32 types (not error/never) in the frozen TyCtx,
// confirming the ADT definition is usable by MIR lowering.
#[test]
fn test_lowering_uses_adt_def_for_field_access() {
    let (frozen_ctx, (hir, adt_id)) = with_fresh_ty_ctx(|ctx| {
        make_hir_with_struct("Point", vec!["x", "y"], ctx)
    });

    let lower_ctx = PipelineLowerCtx::new(&frozen_ctx, &hir);
    let def: AdtDef = lower_ctx.adt_def(adt_id);

    // Verify we got a real definition (not an empty stub)
    assert_eq!(def.variants.len(), 1);
    let fields = &def.variants[0].fields;
    assert_eq!(fields.len(), 2);

    // Verify the field types are concrete and valid
    for &field_ty in fields {
        assert!(!frozen_ctx.ty_is_error(field_ty), "field type should not be error");
        let kind = frozen_ctx.ty_kind(field_ty);
        assert!(
            matches!(kind, glyim_type::TyKind::Int(IntTy::I32)),
            "field type should be i32, got {}",
            glyim_type::PrintTy::new(field_ty, &frozen_ctx)
        );
    }

    // Verify the definition is recognized as a Struct
    assert!(matches!(def.kind, AdtKind::Struct));
}
