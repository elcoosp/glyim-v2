use glyim_core::def_id::{AdtId, ClosureId, FnDefId, OpaqueTyId, TraitDefId};
use glyim_core::primitives::{Abi, FloatTy, IntTy, Mutability, Safety, UintTy};

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

// ===================== V06 Core Test Plan =====================

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
        "manual impl Send should override auto deduction"
    );
    assert!(
        !flags.contains(AutoTraitFlags::SYNC),
        "no manual Sync impl, and raw ptr means no auto Sync"
    );
}

// ===================== Primitives =====================

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

#[test]
fn char_type_has_all_auto_traits() {
    let (ctx, char_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| c.mk_ty(TyKind::Char));
    assert!(
        ctx.auto_trait_flags(char_ty)
            .contains(AutoTraitFlags::all())
    );
}

#[test]
fn string_type_has_all_auto_traits() {
    let (ctx, str_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| c.mk_ty(TyKind::String));
    assert!(ctx.auto_trait_flags(str_ty).contains(AutoTraitFlags::all()));
}

#[test]
fn float_types_have_all_auto_traits() {
    let (ctx, f32_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| c.mk_ty(TyKind::Float(FloatTy::F32)));
    assert!(ctx.auto_trait_flags(f32_ty).contains(AutoTraitFlags::all()));
    let (ctx2, f64_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| c.mk_ty(TyKind::Float(FloatTy::F64)));
    assert!(
        ctx2.auto_trait_flags(f64_ty)
            .contains(AutoTraitFlags::all())
    );
}

#[test]
fn never_type_has_all_auto_traits() {
    let ctx = test_frozen_ty_ctx();
    assert!(
        ctx.auto_trait_flags(Ty::NEVER)
            .contains(AutoTraitFlags::all())
    );
}

// ===================== Error / Infer / Param / Bound =====================

#[test]
fn error_type_has_no_auto_traits() {
    let ctx = test_frozen_ty_ctx();
    let flags = ctx.auto_trait_flags(Ty::ERROR);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(!flags.contains(AutoTraitFlags::UNPIN));
}

#[test]
fn infer_type_has_no_auto_traits() {
    let (ctx, infer_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let var = TyVar::from_raw(0);
        c.mk_ty(TyKind::Infer(InferVar::Ty(var)))
    });
    let flags = ctx.auto_trait_flags(infer_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(!flags.contains(AutoTraitFlags::UNPIN));
}

#[test]
fn param_type_has_no_auto_traits() {
    let (ctx, param_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let name = c.resolver().intern("T");
        c.mk_ty(TyKind::Param(ParamTy { index: 0, name }))
    });
    let flags = ctx.auto_trait_flags(param_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(!flags.contains(AutoTraitFlags::UNPIN));
}

#[test]
fn bound_type_has_no_auto_traits() {
    let (ctx, bound_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        c.mk_ty(TyKind::Bound(
            0,
            BoundTy {
                var: 0,
                kind: BoundTyKind::Anon,
            },
        ))
    });
    let flags = ctx.auto_trait_flags(bound_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(!flags.contains(AutoTraitFlags::UNPIN));
}

// ===================== Raw pointers =====================

#[test]
fn raw_ptr_is_unpin_only() {
    let (ctx, raw_ptr_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let inner = ctx_mut.bool_ty();
        ctx_mut.mk_ty(TyKind::RawPtr(inner, Mutability::Not))
    });
    let flags = ctx.auto_trait_flags(raw_ptr_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(flags.contains(AutoTraitFlags::UNPIN));

    let (ctx2, raw_mut_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let inner = ctx_mut.bool_ty();
        ctx_mut.mk_ty(TyKind::RawPtr(inner, Mutability::Mut))
    });
    let flags2 = ctx2.auto_trait_flags(raw_mut_ty);
    assert!(!flags2.contains(AutoTraitFlags::SEND));
    assert!(!flags2.contains(AutoTraitFlags::SYNC));
    assert!(flags2.contains(AutoTraitFlags::UNPIN));
}

// ===================== Tuples =====================

#[test]
fn tuple_auto_traits_are_intersection() {
    let (ctx, tuple_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let bool_ty = ctx_mut.bool_ty();
        let raw_ptr_ty = ctx_mut.mk_ty(TyKind::RawPtr(bool_ty, Mutability::Not));
        let substs =
            ctx_mut.intern_substitution(vec![GenericArg::Ty(bool_ty), GenericArg::Ty(raw_ptr_ty)]);
        ctx_mut.mk_tuple(substs)
    });
    let flags = ctx.auto_trait_flags(tuple_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(flags.contains(AutoTraitFlags::UNPIN));
}

#[test]
fn empty_tuple_has_all_auto_traits() {
    let (ctx, unit_tuple) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let substs = c.intern_substitution(vec![]);
        c.mk_tuple(substs)
    });
    assert!(
        ctx.auto_trait_flags(unit_tuple)
            .contains(AutoTraitFlags::all())
    );
}

// ===================== Slice / Array =====================

#[test]
fn slice_inherits_element_auto_traits() {
    let (ctx, slice_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let bool_ty = ctx_mut.bool_ty();
        ctx_mut.mk_ty(TyKind::Slice(bool_ty))
    });
    assert!(
        ctx.auto_trait_flags(slice_ty)
            .contains(AutoTraitFlags::all())
    );
}

#[test]
fn slice_of_raw_ptr_is_not_send() {
    let (ctx, slice_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let raw_ptr_ty = c.mk_ty(TyKind::RawPtr(c.bool_ty(), Mutability::Not));
        c.mk_ty(TyKind::Slice(raw_ptr_ty))
    });
    let flags = ctx.auto_trait_flags(slice_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(flags.contains(AutoTraitFlags::UNPIN));
}

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
    assert!(
        ctx.auto_trait_flags(array_ty)
            .contains(AutoTraitFlags::all())
    );
}

#[test]
fn array_of_raw_ptr_is_not_send() {
    let (ctx, array_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let bool_ty = c.bool_ty();
        let raw_ptr_ty = c.mk_ty(TyKind::RawPtr(bool_ty, Mutability::Not));
        let const_val = Const {
            kind: ConstKind::Uint(3),
            ty: bool_ty,
        };
        c.mk_ty(TyKind::Array(raw_ptr_ty, const_val))
    });
    let flags = ctx.auto_trait_flags(array_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(flags.contains(AutoTraitFlags::UNPIN));
}

// ===================== ADT =====================

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
    assert!(ctx.auto_trait_flags(adt_ty).contains(AutoTraitFlags::all()));
}

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

#[test]
fn adt_with_no_repr_gets_no_auto_traits() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(500);
        let substs = ctx_mut.intern_substitution(vec![]);
        ctx_mut.mk_adt(adt_id, substs)
    });
    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(!flags.contains(AutoTraitFlags::UNPIN));
}

#[test]
fn adt_with_empty_fields_has_all_auto_traits() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(501);
        let substs = ctx_mut.intern_substitution(vec![]);
        ctx_mut.register_adt_repr(adt_id, vec![]);
        ctx_mut.mk_adt(adt_id, substs)
    });
    assert!(ctx.auto_trait_flags(adt_ty).contains(AutoTraitFlags::all()));
}

// ===================== Negative impls =====================

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

#[test]
fn negative_impl_sync_doesnt_affect_send() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(300);
        let substs = ctx_mut.intern_substitution(vec![]);
        let field_ty = ctx_mut.bool_ty();
        ctx_mut.register_adt_repr(adt_id, vec![field_ty]);
        ctx_mut.register_negative_impl(adt_id, AutoTrait::Sync);
        ctx_mut.mk_adt(adt_id, substs)
    });
    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(flags.contains(AutoTraitFlags::UNPIN));
}

#[test]
fn negative_impl_unpin_doesnt_affect_send_or_sync() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(301);
        let substs = ctx_mut.intern_substitution(vec![]);
        let field_ty = ctx_mut.bool_ty();
        ctx_mut.register_adt_repr(adt_id, vec![field_ty]);
        ctx_mut.register_negative_impl(adt_id, AutoTrait::Unpin);
        ctx_mut.mk_adt(adt_id, substs)
    });
    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(flags.contains(AutoTraitFlags::SEND));
    assert!(flags.contains(AutoTraitFlags::SYNC));
    assert!(!flags.contains(AutoTraitFlags::UNPIN));
}

#[test]
fn multiple_negative_impls() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(302);
        let substs = ctx_mut.intern_substitution(vec![]);
        let field_ty = ctx_mut.bool_ty();
        ctx_mut.register_adt_repr(adt_id, vec![field_ty]);
        ctx_mut.register_negative_impl(adt_id, AutoTrait::Send);
        ctx_mut.register_negative_impl(adt_id, AutoTrait::Sync);
        ctx_mut.mk_adt(adt_id, substs)
    });
    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(flags.contains(AutoTraitFlags::UNPIN));
}

// ===================== Manual impls =====================

#[test]
fn manual_impl_sync_on_raw_ptr_struct() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(303);
        let substs = ctx_mut.intern_substitution(vec![]);
        let bool_ty = ctx_mut.bool_ty();
        let raw_ptr_ty = ctx_mut.mk_ty(TyKind::RawPtr(bool_ty, Mutability::Not));
        ctx_mut.register_adt_repr(adt_id, vec![raw_ptr_ty]);
        ctx_mut.register_manual_impl(adt_id, AutoTrait::Sync);
        ctx_mut.mk_adt(adt_id, substs)
    });
    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(flags.contains(AutoTraitFlags::SYNC));
}

#[test]
fn manual_impl_overrides_negative_impl() {
    let (ctx, adt_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(304);
        let substs = ctx_mut.intern_substitution(vec![]);
        let field_ty = ctx_mut.bool_ty();
        ctx_mut.register_adt_repr(adt_id, vec![field_ty]);
        ctx_mut.register_negative_impl(adt_id, AutoTrait::Send);
        ctx_mut.register_manual_impl(adt_id, AutoTrait::Send);
        ctx_mut.mk_adt(adt_id, substs)
    });
    let flags = ctx.auto_trait_flags(adt_ty);
    assert!(
        flags.contains(AutoTraitFlags::SEND),
        "manual impl should override negative impl"
    );
}

// ===================== Nested ADTs =====================

#[test]
fn nested_adt_auto_traits() {
    let (ctx, outer_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let inner_adt_id = AdtId::from_raw(400);
        let inner_substs = ctx_mut.intern_substitution(vec![]);
        let bool_ty = ctx_mut.bool_ty();
        ctx_mut.register_adt_repr(inner_adt_id, vec![bool_ty]);
        let inner_ty = ctx_mut.mk_adt(inner_adt_id, inner_substs);
        let outer_adt_id = AdtId::from_raw(401);
        let outer_substs = ctx_mut.intern_substitution(vec![]);
        ctx_mut.register_adt_repr(outer_adt_id, vec![inner_ty]);
        ctx_mut.mk_adt(outer_adt_id, outer_substs)
    });
    assert!(
        ctx.auto_trait_flags(outer_ty)
            .contains(AutoTraitFlags::all())
    );
}

#[test]
fn nested_adt_inner_not_send_outer_not_send() {
    let (ctx, outer_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let bool_ty = ctx_mut.bool_ty();
        let raw_ptr_ty = ctx_mut.mk_ty(TyKind::RawPtr(bool_ty, Mutability::Not));
        let inner_adt_id = AdtId::from_raw(402);
        let inner_substs = ctx_mut.intern_substitution(vec![]);
        ctx_mut.register_adt_repr(inner_adt_id, vec![raw_ptr_ty]);
        let inner_ty = ctx_mut.mk_adt(inner_adt_id, inner_substs);
        let outer_adt_id = AdtId::from_raw(403);
        let outer_substs = ctx_mut.intern_substitution(vec![]);
        ctx_mut.register_adt_repr(outer_adt_id, vec![inner_ty]);
        ctx_mut.mk_adt(outer_adt_id, outer_substs)
    });
    let flags = ctx.auto_trait_flags(outer_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(flags.contains(AutoTraitFlags::UNPIN));
}

// ===================== Coinductive =====================

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

// ===================== FnPtr / FnDef / Closure =====================

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
    assert!(
        ctx.auto_trait_flags(fn_ptr_ty)
            .contains(AutoTraitFlags::all())
    );
}

#[test]
fn fn_def_has_all_auto_traits() {
    let (ctx, fn_def_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let substs = c.intern_substitution(vec![]);
        c.mk_ty(TyKind::FnDef(FnDefId::from_raw(0), substs))
    });
    assert!(
        ctx.auto_trait_flags(fn_def_ty)
            .contains(AutoTraitFlags::all())
    );
}

#[test]
fn closure_with_all_send_fields_is_send() {
    let (ctx, closure_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let bool_ty = c.bool_ty();
        let substs = c.intern_substitution(vec![GenericArg::Ty(bool_ty)]);
        c.mk_ty(TyKind::Closure(ClosureId::from_raw(0), substs))
    });
    assert!(
        ctx.auto_trait_flags(closure_ty)
            .contains(AutoTraitFlags::all())
    );
}

#[test]
fn closure_with_raw_ptr_is_not_send() {
    let (ctx, closure_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let bool_ty = c.bool_ty();
        let raw_ptr_ty = c.mk_ty(TyKind::RawPtr(bool_ty, Mutability::Not));
        let substs = c.intern_substitution(vec![GenericArg::Ty(raw_ptr_ty)]);
        c.mk_ty(TyKind::Closure(ClosureId::from_raw(1), substs))
    });
    let flags = ctx.auto_trait_flags(closure_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(flags.contains(AutoTraitFlags::UNPIN));
}

// ===================== Dynamic / Opaque / Projection =====================

#[test]
fn dynamic_type_has_no_auto_traits() {
    let (ctx, dyn_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let preds: Box<[Predicate]> = Box::new([]);
        let bound_vars = Box::new([]);
        let binder = Binder::bind(preds, bound_vars);
        c.mk_ty(TyKind::Dynamic(binder, Region::Static))
    });
    let flags = ctx.auto_trait_flags(dyn_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
}

#[test]
fn opaque_type_has_no_auto_traits() {
    let (ctx, opaque_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let substs = c.intern_substitution(vec![]);
        c.mk_ty(TyKind::Opaque(OpaqueTyId::from_raw(0), substs))
    });
    let flags = ctx.auto_trait_flags(opaque_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
}

#[test]
fn projection_type_has_no_auto_traits() {
    let (ctx, proj_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let substs = c.intern_substitution(vec![GenericArg::Ty(c.bool_ty())]);
        let trait_ref = TraitRef {
            def_id: TraitDefId::from_raw(0),
            substs,
        };
        let name = c.resolver().intern("Item");
        c.mk_ty(TyKind::Projection(ProjectionTy {
            trait_ref,
            item_name: name,
        }))
    });
    let flags = ctx.auto_trait_flags(proj_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
}

// ===================== Ref edge cases =====================

#[test]
fn ref_mut_to_non_sync_is_not_send() {
    let (ctx, ref_mut_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let bool_ty = ctx_mut.bool_ty();
        let raw_ptr_ty = ctx_mut.mk_ty(TyKind::RawPtr(bool_ty, Mutability::Not));
        let adt_id = AdtId::from_raw(404);
        let substs = ctx_mut.intern_substitution(vec![]);
        ctx_mut.register_adt_repr(adt_id, vec![raw_ptr_ty]);
        let non_sync_ty = ctx_mut.mk_adt(adt_id, substs);
        ctx_mut.mk_ref(Region::Erased, non_sync_ty, Mutability::Mut)
    });
    let flags = ctx.auto_trait_flags(ref_mut_ty);
    assert!(!flags.contains(AutoTraitFlags::SEND));
    assert!(!flags.contains(AutoTraitFlags::SYNC));
    assert!(flags.contains(AutoTraitFlags::UNPIN));
}

#[test]
fn ref_to_send_but_not_sync_is_not_send() {
    let (ctx, ref_ty) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let bool_ty = ctx_mut.bool_ty();
        let raw_ptr_ty = ctx_mut.mk_ty(TyKind::RawPtr(bool_ty, Mutability::Not));
        let adt_id = AdtId::from_raw(405);
        let substs = ctx_mut.intern_substitution(vec![]);
        ctx_mut.register_adt_repr(adt_id, vec![raw_ptr_ty]);
        ctx_mut.register_manual_impl(adt_id, AutoTrait::Send);
        let send_but_not_sync = ctx_mut.mk_adt(adt_id, substs);
        ctx_mut.mk_ref(Region::Erased, send_but_not_sync, Mutability::Not)
    });
    let flags = ctx.auto_trait_flags(ref_ty);
    assert!(
        !flags.contains(AutoTraitFlags::SEND),
        "&T where T: !Sync should not be Send"
    );
    assert!(
        !flags.contains(AutoTraitFlags::SYNC),
        "&T where T: !Sync should not be Sync"
    );
}

// ===================== Convenience methods =====================

#[test]
fn implements_auto_trait_convenience_method() {
    let ctx = test_frozen_ty_ctx();
    assert!(ctx.implements_auto_trait(Ty::BOOL, AutoTrait::Send));
    assert!(ctx.implements_auto_trait(Ty::BOOL, AutoTrait::Sync));
    assert!(ctx.implements_auto_trait(Ty::BOOL, AutoTrait::Unpin));
    assert!(!ctx.implements_auto_trait(Ty::ERROR, AutoTrait::Send));
    assert!(!ctx.implements_auto_trait(Ty::ERROR, AutoTrait::Sync));
}

#[test]
fn frozen_context_negative_and_manual_impl_queries() {
    let (ctx, _) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(600);
        ctx_mut.register_negative_impl(adt_id, AutoTrait::Send);
        ctx_mut.register_manual_impl(adt_id, AutoTrait::Sync);
    });
    let adt_id = AdtId::from_raw(600);
    assert!(ctx.has_negative_impl(adt_id, AutoTrait::Send));
    assert!(!ctx.has_negative_impl(adt_id, AutoTrait::Sync));
    assert!(ctx.has_manual_impl(adt_id, AutoTrait::Sync));
    assert!(!ctx.has_manual_impl(adt_id, AutoTrait::Send));
}

#[test]
fn frozen_context_adt_repr_accessor() {
    let (ctx, _) = with_fresh_ty_ctx(|ctx_mut: &mut TyCtxMut| {
        let adt_id = AdtId::from_raw(700);
        let bool_ty = ctx_mut.bool_ty();
        ctx_mut.register_adt_repr(adt_id, vec![bool_ty]);
    });
    let adt_id = AdtId::from_raw(700);
    let repr = ctx.adt_repr(adt_id);
    assert!(repr.is_some());
    assert_eq!(repr.unwrap().field_tys.len(), 1);
    assert!(ctx.adt_repr(AdtId::from_raw(999)).is_none());
}

#[test]
fn auto_trait_flag_mapping() {
    assert_eq!(AutoTrait::Send.flag(), AutoTraitFlags::SEND);
    assert_eq!(AutoTrait::Sync.flag(), AutoTraitFlags::SYNC);
    assert_eq!(AutoTrait::Unpin.flag(), AutoTraitFlags::UNPIN);
}

#[test]
fn auto_trait_all_constant() {
    assert_eq!(AutoTrait::ALL.len(), 3);
    assert!(AutoTrait::ALL.contains(&AutoTrait::Send));
    assert!(AutoTrait::ALL.contains(&AutoTrait::Sync));
    assert!(AutoTrait::ALL.contains(&AutoTrait::Unpin));
}

// ===================== Helpers =====================

fn ctx_bool_ty() -> Ty {
    let ctx = super::helpers::test_ty_ctx();
    ctx.bool_ty()
}

fn ctx_i32_ty() -> Ty {
    let mut ctx = super::helpers::test_ty_ctx();
    ctx.mk_ty(TyKind::Int(IntTy::I32))
}
