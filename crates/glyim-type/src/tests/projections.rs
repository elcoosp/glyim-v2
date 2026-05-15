//! Tests for associated type projections.
use glyim_core::def_id::TraitDefId;
use glyim_core::primitives::IntTy;

use crate::{
    GenericArg, InferVar, PrintTy, ProjectionTy, TraitRef, TyCtxMut, TyKind, TyVar, TypeFlags,
};

use super::test_helpers::with_fresh_ty_ctx;

/// Helper: build a projection type `<self_ty as trait_def_id>::item_name`
fn make_projection(
    ctx: &mut TyCtxMut,
    trait_def_id: TraitDefId,
    self_ty: crate::Ty,
    item_name: &str,
) -> crate::Ty {
    let name = ctx.resolver().intern(item_name);
    let substs = ctx.intern_substitution(vec![GenericArg::Ty(self_ty)]);
    let trait_ref = TraitRef {
        def_id: trait_def_id,
        substs,
    };
    let proj = ProjectionTy {
        trait_ref,
        item_name: name,
    };
    ctx.mk_ty(TyKind::Projection(proj))
}

#[test]
fn projection_type_construction() {
    let (ctx, proj_ty) = with_fresh_ty_ctx(|ctx| {
        let trait_id = TraitDefId::from_raw(0);
        let self_ty = ctx.bool_ty();
        make_projection(ctx, trait_id, self_ty, "Item")
    });
    assert!(!ctx.ty_is_error(proj_ty));
    let kind = ctx.ty_kind(proj_ty);
    assert!(matches!(kind, TyKind::Projection(_)));
}

#[test]
fn projection_type_flags() {
    let (ctx, proj_ty) = with_fresh_ty_ctx(|ctx| {
        let trait_id = TraitDefId::from_raw(0);
        let self_ty = ctx.bool_ty();
        make_projection(ctx, trait_id, self_ty, "Item")
    });
    let flags = ctx.ty_flags(proj_ty);
    assert!(!flags.contains(TypeFlags::HAS_TY_INFER));
    assert!(!flags.contains(TypeFlags::HAS_TY_PARAM));
}

#[test]
fn projection_type_flags_with_infer() {
    let (ctx, proj_ty) = with_fresh_ty_ctx(|ctx| {
        let trait_id = TraitDefId::from_raw(0);
        let infer_var = ctx.mk_ty(TyKind::Infer(InferVar::Ty(TyVar::from_raw(0))));
        make_projection(ctx, trait_id, infer_var, "Item")
    });
    let flags = ctx.ty_flags(proj_ty);
    assert!(flags.contains(TypeFlags::HAS_TY_INFER));
}

#[test]
fn projection_display() {
    let (ctx, proj_ty) = with_fresh_ty_ctx(|ctx| {
        let trait_id = TraitDefId::from_raw(0);
        let self_ty = ctx.bool_ty();
        make_projection(ctx, trait_id, self_ty, "Item")
    });
    let display = format!("{}", PrintTy::new(proj_ty, &ctx));
    assert!(display.contains("bool"));
    assert!(display.contains("Trait0"));
    assert!(display.contains("Item"));
}

#[test]
fn projection_equality() {
    let (ctx, (p1, p2, p3)) = with_fresh_ty_ctx(|ctx| {
        let trait_id = TraitDefId::from_raw(1);
        let self_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
        let item = ctx.resolver().intern("Item");
        let substs = ctx.intern_substitution(vec![GenericArg::Ty(self_ty)]);
        let trait_ref = TraitRef {
            def_id: trait_id,
            substs,
        };
        let proj1 = ctx.mk_ty(TyKind::Projection(ProjectionTy {
            trait_ref: trait_ref.clone(),
            item_name: item,
        }));
        let proj2 = ctx.mk_ty(TyKind::Projection(ProjectionTy {
            trait_ref: trait_ref.clone(),
            item_name: item,
        }));
        let proj3 = ctx.mk_ty(TyKind::Projection(ProjectionTy {
            trait_ref,
            item_name: ctx.resolver().intern("Other"),
        }));
        (proj1, proj2, proj3)
    });
    // Ty indices differ, but underlying TyKinds are equal
    assert_eq!(ctx.ty_kind(p1), ctx.ty_kind(p2));
    assert_ne!(ctx.ty_kind(p1), ctx.ty_kind(p3));
}
