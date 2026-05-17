use crate::*;
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
