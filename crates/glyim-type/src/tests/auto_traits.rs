use glyim_core::def_id::AdtId;
use glyim_core::primitives::{Abi, IntTy, Mutability, Safety, UintTy};

use super::helpers::{test_frozen_ty_ctx, with_fresh_ty_ctx};
use crate::auto_trait::{AutoTrait, AutoTraitFlags};
use crate::const_val::{Const, ConstKind};
use crate::fn_sig::FnSig;
use crate::region::Region;
use crate::substitution::GenericArg;
use crate::ty::TyKind;
use crate::*;

/// Helper: create an ADT with given field types, returning (frozen_ctx, adt_ty)
fn make_single_field_adt(field_ty: Ty) -> (TyCtx, Ty) {
    with_fresh_ty_ctx(|ctx: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(100);
        let substs = ctx.intern_substitution(vec![]);
        ctx.register_adt_repr(adt_id, vec![field_ty]);
        ctx.mk_adt(adt_id, substs)
    })
}

/// Helper: create an ADT with multiple field types
#[allow(dead_code)]
fn make_multi_field_adt(field_tys: Vec<Ty>) -> (TyCtx, Ty) {
    with_fresh_ty_ctx(|ctx: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(101);
        let substs = ctx.intern_substitution(vec![]);
        ctx.register_adt_repr(adt_id, field_tys);
        ctx.mk_adt(adt_id, substs)
    })
}

/// V06-T01: Struct with all Send fields → auto impl Send (run-pass)
#[test]
fn struct_all_send_fields_auto_impl_send() {
    let (ctx, adt_ty) = make_single_field_adt(ctx_bool_ty());
    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(
        flags.contains(AutoTraitFlags::SEND),
        "struct with all-Send fields should auto-impl Send"
    );
    assert!(
        flags.contains(AutoTraitFlags::SYNC),
        "struct with all-Send+Sync fields should auto-impl Sync"
    );
    assert!(
        flags.contains(AutoTraitFlags::UNPIN),
        "struct with all-Unpin fields should auto-impl Unpin"
    );
}

/// V06-T01 extended: struct with i32 field → all auto traits
#[test]
fn struct_with_int_field_auto_impls_all() {
    let (ctx, adt_ty) = make_single_field_adt(ctx_i32_ty());
    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(flags.contains(AutoTraitFlags::SEND));
    assert!(flags.contains(AutoTraitFlags::SYNC));
    assert!(flags.contains(AutoTraitFlags::UNPIN));
}

/// V06-T02: Struct with non-Send field → no auto Send
#[test]
fn struct_with_non_send_field_no_auto_send() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let inner = ctx_mut.bool_ty();
        let raw_ptr_ty = ctx_mut.mk_ty(TyKind::RawPtr(inner, Mutability::Not));
        let adt_id = AdtId::from_raw(102);
        let substs = ctx_mut.intern_substitution(vec![]);
        ctx_mut.register_adt_repr(adt_id, vec![raw_ptr_ty]);
        ctx_mut.mk_adt(adt_id, substs)
    });

    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(
        !flags.contains(AutoTraitFlags::SEND),
        "struct with *const T field should NOT auto-impl Send"
    );
    assert!(
        !flags.contains(AutoTraitFlags::SYNC),
        "struct with *const T field should NOT auto-impl Sync"
    );
    assert!(
        flags.contains(AutoTraitFlags::UNPIN),
        "struct with *const T field should still auto-impl Unpin"
    );
}

/// V06-T03: Negative impl `impl !Send for T` → overrides auto
#[test]
fn negative_impl_overrides_auto_send() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(103);
        let substs = ctx_mut.intern_substitution(vec![]);
        let field_ty = ctx_mut.bool_ty();
        ctx_mut.register_adt_repr(adt_id, vec![field_ty]);
        ctx_mut.register_negative_impl(adt_id, AutoTrait::Send);
        ctx_mut.mk_adt(adt_id, substs)
    });

    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(
        !flags.contains(AutoTraitFlags::SEND),
        "negative impl !Send should prevent auto Send"
    );
    assert!(
        flags.contains(AutoTraitFlags::SYNC),
        "negative impl !Send should NOT prevent auto Sync"
    );
    assert!(
        flags.contains(AutoTraitFlags::UNPIN),
        "negative impl !Send should NOT prevent auto Unpin"
    );
}

/// V06-T04: Transitive auto traits — &T is Send if T: Sync, &mut T is Send if T: Send
#[test]
fn ref_send_requires_inner_sync() {
    let (ctx, ref_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let inner = ctx_mut.bool_ty();
        ctx_mut.mk_ref(Region::Erased, inner, Mutability::Not)
    });

    let flags = ctx.auto_trait_flags(ref_ty);
    assert!(
        flags.contains(AutoTraitFlags::SEND),
        "&bool should be Send (bool: Sync)"
    );
    assert!(
        flags.contains(AutoTraitFlags::SYNC),
        "&bool should be Sync (bool: Sync)"
    );
}

#[test]
fn ref_mut_send_requires_inner_send() {
    let (ctx, ref_mut_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let inner = ctx_mut.bool_ty();
        ctx_mut.mk_ref(Region::Erased, inner, Mutability::Mut)
    });

    let flags = ctx.auto_trait_flags(ref_mut_ty);
    assert!(
        flags.contains(AutoTraitFlags::SEND),
        "&mut bool should be Send (bool: Send)"
    );
    assert!(
        flags.contains(AutoTraitFlags::SYNC),
        "&mut bool should be Sync (bool: Sync)"
    );
}

#[test]
fn ref_to_non_sync_is_not_send() {
    let (ctx, ref_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let inner_bool = ctx_mut.bool_ty();
        let raw_ptr_ty = ctx_mut.mk_ty(TyKind::RawPtr(inner_bool, Mutability::Not));
        let adt_id = AdtId::from_raw(104);
        let substs = ctx_mut.intern_substitution(vec![]);
        ctx_mut.register_adt_repr(adt_id, vec![raw_ptr_ty]);
        let non_sync_ty = ctx_mut.mk_adt(adt_id, substs);

        ctx_mut.mk_ref(Region::Erased, non_sync_ty, Mutability::Not)
    });

    let flags = ctx.auto_trait_flags(ref_ty);
    assert!(
        !flags.contains(AutoTraitFlags::SEND),
        "&NonSync should NOT be Send"
    );
    assert!(
        !flags.contains(AutoTraitFlags::SYNC),
        "&NonSync should NOT be Sync"
    );
}

/// V06-T05: Manual impl of auto trait overrides auto
#[test]
fn manual_impl_overrides_auto() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(105);
        let substs = ctx_mut.intern_substitution(vec![]);
        let inner = ctx_mut.bool_ty();
        let raw_ptr_ty = ctx_mut.mk_ty(TyKind::RawPtr(inner, Mutability::Not));
        ctx_mut.register_adt_repr(adt_id, vec![raw_ptr_ty]);
        ctx_mut.register_manual_impl(adt_id, AutoTrait::Send);
        ctx_mut.mk_adt(adt_id, substs)
    });

    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(
        flags.contains(AutoTraitFlags::SEND),
        "manual impl Send should override auto deduction (even with raw ptr)"
    );
    assert!(
        !flags.contains(AutoTraitFlags::SYNC),
        "no manual Sync impl, and raw ptr means no auto Sync"
    );
}

/// Primitives: bool, never, unit all have all auto traits
#[test]
fn primitives_have_all_auto_traits() {
    let ctx = test_frozen_ty_ctx();

    assert!(
        ctx.auto_trait_flags(Ty::BOOL)
            .contains(AutoTraitFlags::all())
    );
    assert!(
        ctx.auto_trait_flags(Ty::NEVER)
            .contains(AutoTraitFlags::all())
    );
    assert!(
        ctx.auto_trait_flags(Ty::UNIT)
            .contains(AutoTraitFlags::all())
    );
}

#[test]
fn int_types_have_all_auto_traits() {
    let (ctx, i32_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| c.mk_ty(TyKind::Int(IntTy::I32)));
    assert!(ctx.auto_trait_flags(i32_ty).contains(AutoTraitFlags::all()));

    let (ctx2, u64_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| c.mk_ty(TyKind::Uint(UintTy::U64)));
    assert!(
        ctx2.auto_trait_flags(u64_ty)
            .contains(AutoTraitFlags::all())
    );
}

/// Raw pointers: not Send, not Sync, but Unpin
#[test]
fn raw_ptr_is_unpin_only() {
    let (ctx, raw_ptr_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let inner = ctx_mut.bool_ty();
        ctx_mut.mk_ty(TyKind::RawPtr(inner, Mutability::Not))
    });

    let flags = ctx.auto_trait_flags(raw_ptr_ty);
    assert!(
        !flags.contains(AutoTraitFlags::SEND),
        "*const T is not Send"
    );
    assert!(
        !flags.contains(AutoTraitFlags::SYNC),
        "*const T is not Sync"
    );
    assert!(flags.contains(AutoTraitFlags::UNPIN), "*const T is Unpin");

    let (ctx2, raw_mut_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let inner = ctx_mut.bool_ty();
        ctx_mut.mk_ty(TyKind::RawPtr(inner, Mutability::Mut))
    });

    let flags2 = ctx2.auto_trait_flags(raw_mut_ty);
    assert!(!flags2.contains(AutoTraitFlags::SEND), "*mut T is not Send");
    assert!(!flags2.contains(AutoTraitFlags::SYNC), "*mut T is not Sync");
    assert!(flags2.contains(AutoTraitFlags::UNPIN), "*mut T is Unpin");
}

/// Tuples: auto traits are intersection of element auto traits
#[test]
fn tuple_auto_traits_are_intersection() {
    let (ctx, tuple_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let bool_ty = ctx_mut.bool_ty();
        let inner = bool_ty;
        let raw_ptr_ty = ctx_mut.mk_ty(TyKind::RawPtr(inner, Mutability::Not));
        let substs =
            ctx_mut.intern_substitution(vec![GenericArg::Ty(bool_ty), GenericArg::Ty(raw_ptr_ty)]);
        ctx_mut.mk_tuple(substs)
    });

    let flags = ctx.auto_trait_flags(tuple_ty);
    assert!(
        !flags.contains(AutoTraitFlags::SEND),
        "(bool, *const T) should NOT be Send"
    );
    assert!(
        !flags.contains(AutoTraitFlags::SYNC),
        "(bool, *const T) should NOT be Sync"
    );
    assert!(
        flags.contains(AutoTraitFlags::UNPIN),
        "(bool, *const T) should be Unpin"
    );
}

/// Slice: inherits auto traits from element type
#[test]
fn slice_inherits_element_auto_traits() {
    let (ctx, slice_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let bool_ty = ctx_mut.bool_ty();
        ctx_mut.mk_ty(TyKind::Slice(bool_ty))
    });

    let flags = ctx.auto_trait_flags(slice_ty);
    assert!(
        flags.contains(AutoTraitFlags::all()),
        "[bool] should have all auto traits"
    );
}

/// Array: inherits auto traits from element type
#[test]
fn array_inherits_element_auto_traits() {
    let (ctx, array_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let bool_ty = ctx_mut.bool_ty();
        let const_val = Const {
            kind: ConstKind::Uint(3),
            ty: bool_ty,
        };
        ctx_mut.mk_ty(TyKind::Array(bool_ty, const_val))
    });

    let flags = ctx.auto_trait_flags(array_ty);
    assert!(
        flags.contains(AutoTraitFlags::all()),
        "[bool; 3] should have all auto traits"
    );
}

/// Multi-field ADT: all fields must implement the auto trait
#[test]
fn multi_field_adt_all_must_implement() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let bool_ty = ctx_mut.bool_ty();
        let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
        let adt_id = AdtId::from_raw(106);
        let substs = ctx_mut.intern_substitution(vec![]);
        ctx_mut.register_adt_repr(adt_id, vec![bool_ty, i32_ty]);
        ctx_mut.mk_adt(adt_id, substs)
    });

    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(flags.contains(AutoTraitFlags::all()));
}

/// Multi-field ADT: one bad field spoils the auto trait
#[test]
fn multi_field_adt_one_bad_field_removes_send() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let bool_ty = ctx_mut.bool_ty();
        let raw_ptr_ty = ctx_mut.mk_ty(TyKind::RawPtr(bool_ty, Mutability::Not));
        let adt_id = AdtId::from_raw(107);
        let substs = ctx_mut.intern_substitution(vec![]);
        ctx_mut.register_adt_repr(adt_id, vec![bool_ty, raw_ptr_ty]);
        ctx_mut.mk_adt(adt_id, substs)
    });

    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(flags.contains(AutoTraitFlags::UNPIN));
}

/// Negative impl on one trait doesn't affect others
#[test]
fn negative_impl_send_doesnt_affect_sync_or_unpin() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(108);
        let substs = ctx_mut.intern_substitution(vec![]);
        let field_ty = ctx_mut.bool_ty();
        ctx_mut.register_adt_repr(adt_id, vec![field_ty]);
        ctx_mut.register_negative_impl(adt_id, AutoTrait::Send);
        ctx_mut.mk_adt(adt_id, substs)
    });

    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(flags.contains(AutoTraitFlags::SYNC));
    assert!(flags.contains(AutoTraitFlags::UNPIN));
}

/// Coinductive: recursive ADT type should still compute auto traits
#[test]
fn coinductive_recursive_adt() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(200);
        let substs = ctx_mut.intern_substitution(vec![]);
        let adt_ty = ctx_mut.mk_adt(adt_id, substs);
        ctx_mut.register_adt_repr(adt_id, vec![adt_ty]);
        adt_ty
    });

    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(
        flags.contains(AutoTraitFlags::SEND),
        "coinductive: recursive ADT should be Send"
    );
    assert!(
        flags.contains(AutoTraitFlags::SYNC),
        "coinductive: recursive ADT should be Sync"
    );
    assert!(
        flags.contains(AutoTraitFlags::UNPIN),
        "coinductive: recursive ADT should be Unpin"
    );
}

/// FnPtr has all auto traits
#[test]
fn fn_ptr_has_all_auto_traits() {
    let (ctx, fn_ptr_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let bool_ty = ctx_mut.bool_ty();
        let inputs = ctx_mut.intern_substitution(vec![GenericArg::Ty(bool_ty)]);
        let sig = FnSig {
            inputs,
            output: bool_ty,
            c_variadic: false,
            unsafety: Safety::Safe,
            abi: Abi::Glyim,
        };
        ctx_mut.mk_fn_ptr(sig)
    });

    let flags = ctx.auto_trait_flags(fn_ptr_ty);
    assert!(
        flags.contains(AutoTraitFlags::all()),
        "fn ptr should have all auto traits"
    );
}

/// Helper to get a bool Ty from a fresh context
fn ctx_bool_ty() -> Ty {
    let ctx = super::helpers::test_ty_ctx();
    ctx.bool_ty()
}

/// Helper to get an i32 Ty from a fresh context
fn ctx_i32_ty() -> Ty {
    let mut ctx = super::helpers::test_ty_ctx();
    ctx.mk_ty(TyKind::Int(IntTy::I32))
}
