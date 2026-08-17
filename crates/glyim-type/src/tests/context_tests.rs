use crate::*;
use crate::const_val::Const;
use glyim_core::interner::Interner;
use glyim_core::primitives::*;

#[test]
fn test_is_copy_for_primitives() {
    let mut ctx = TyCtxMut::new(Interner::new());
    let bool_ty = ctx.bool_ty();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let char_ty = ctx.mk_ty(TyKind::Char);

    assert!(ctx.is_copy(bool_ty));
    assert!(ctx.is_copy(i32_ty));
    assert!(ctx.is_copy(char_ty));
}

#[test]
fn test_is_copy_for_refs_is_false() {
    let mut ctx = TyCtxMut::new(Interner::new());
    let inner = ctx.bool_ty();
    let ref_ty = ctx.mk_ref(Region::Erased, inner, Mutability::Not);

    assert!(!ctx.is_copy(ref_ty));
}

#[test]
fn test_is_copy_for_tuple_of_copy() {
    let mut ctx = TyCtxMut::new(Interner::new());
    let bool_ty = ctx.bool_ty();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let substs = ctx.intern_substitution(vec![GenericArg::Ty(bool_ty), GenericArg::Ty(i32_ty)]);
    let tuple_ty = ctx.mk_ty(TyKind::Tuple(substs));

    assert!(ctx.is_copy(tuple_ty));
}

#[test]
fn test_is_sized_for_primitives_and_slices() {
    let mut ctx = TyCtxMut::new(Interner::new());
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let usize_ty = ctx.mk_ty(TyKind::Uint(UintTy::Usize));
    let arr_ty = ctx.mk_ty(TyKind::Array(i32_ty, Const { kind: ConstKind::Uint(3), ty: usize_ty }));
    let slice_ty = ctx.mk_ty(TyKind::Slice(i32_ty));

    let frozen = ctx.freeze();
    assert!(frozen.is_sized(i32_ty), "i32 is Sized");
    assert!(frozen.is_sized(arr_ty), "[i32; 3] is Sized");
    assert!(!frozen.is_sized(slice_ty), "[i32] is NOT Sized");
}

#[test]
fn test_deref_ty_for_refs_and_pointers() {
    let mut ctx = TyCtxMut::new(Interner::new());
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let ref_ty = ctx.mk_ref(Region::Erased, i32_ty, Mutability::Not);
    let ref_mut_ty = ctx.mk_ref(Region::Erased, i32_ty, Mutability::Mut);
    let raw_ty = ctx.mk_ty(TyKind::RawPtr(i32_ty, Mutability::Not));

    assert_eq!(ctx.deref_ty(ref_ty), Some(i32_ty), "&T derefs to T");
    assert_eq!(ctx.deref_ty(ref_mut_ty), Some(i32_ty), "&mut T derefs to T");
    assert_eq!(ctx.deref_ty(raw_ty), Some(i32_ty), "*const T derefs to T");
    assert_eq!(ctx.deref_ty(i32_ty), None, "i32 does not deref");
}
