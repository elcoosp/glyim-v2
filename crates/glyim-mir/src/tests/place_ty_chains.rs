use crate::*;
use glyim_core::arena::IndexVec;
use glyim_core::primitives::{IntTy, Mutability, UintTy};
use glyim_span::Span;
use glyim_type::{
    Const, ConstKind, FieldIdx, GenericArg, Region, Ty, TyCtxMut, TyKind,
};
use glyim_test::with_fresh_ty_ctx;

#[test]
fn chain_ref_deref_roundtrip() {
    let (ctx, (ref_bool, bool_ty)) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let bool_ty = c.bool_ty();
        let ref_bool = c.mk_ref(Region::Erased, bool_ty, Mutability::Not);
        (ref_bool, bool_ty)
    });

    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl { ty: ref_bool, mutability: Mutability::Not, source_info: SourceInfo::new(Span::DUMMY) });

    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: Box::new([ProjectionElem::Deref]),
    };
    assert_eq!(place.ty(&ctx, &locals), bool_ty);
}

#[test]
fn chain_raw_ptr_deref_field() {
    let (ctx, (raw_ptr_ty, i32_ty)) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let i32_ty = c.mk_ty(TyKind::Int(IntTy::I32));
        let u32_ty = c.mk_ty(TyKind::Uint(UintTy::U32));
        let substs = c.intern_substitution(vec![GenericArg::Ty(i32_ty), GenericArg::Ty(u32_ty)]);
        let tuple_ty = c.mk_ty(TyKind::Tuple(substs));
        let raw_ptr_ty = c.mk_ty(TyKind::RawPtr(tuple_ty, Mutability::Mut));
        (raw_ptr_ty, i32_ty)
    });

    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl { ty: raw_ptr_ty, mutability: Mutability::Not, source_info: SourceInfo::new(Span::DUMMY) });

    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: Box::new([
            ProjectionElem::Deref,
            ProjectionElem::Field(FieldIdx::from_raw(0)),
        ]),
    };
    assert_eq!(place.ty(&ctx, &locals), i32_ty);
}

#[test]
fn chain_ref_ref_deref_deref() {
    let (ctx, (double_ref, bool_ty)) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let bool_ty = c.bool_ty();
        let ref1 = c.mk_ref(Region::Erased, bool_ty, Mutability::Not);
        let ref2 = c.mk_ref(Region::Erased, ref1, Mutability::Mut);
        (ref2, bool_ty)
    });

    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl { ty: double_ref, mutability: Mutability::Not, source_info: SourceInfo::new(Span::DUMMY) });

    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: Box::new([ProjectionElem::Deref, ProjectionElem::Deref]),
    };
    assert_eq!(place.ty(&ctx, &locals), bool_ty);
}

#[test]
fn chain_ref_to_tuple_to_ref_to_inner() {
    let (ctx, (ref_tuple, i32_ty)) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let i32_ty = c.mk_ty(TyKind::Int(IntTy::I32));
        let ref_i32 = c.mk_ref(Region::Erased, i32_ty, Mutability::Not);
        let u32_ty = c.mk_ty(TyKind::Uint(UintTy::U32));
        let substs = c.intern_substitution(vec![GenericArg::Ty(ref_i32), GenericArg::Ty(u32_ty)]);
        let tuple_ty = c.mk_ty(TyKind::Tuple(substs));
        let ref_tuple = c.mk_ref(Region::Erased, tuple_ty, Mutability::Not);
        (ref_tuple, i32_ty)
    });

    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl { ty: ref_tuple, mutability: Mutability::Not, source_info: SourceInfo::new(Span::DUMMY) });

    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: Box::new([
            ProjectionElem::Deref,
            ProjectionElem::Field(FieldIdx::from_raw(0)),
            ProjectionElem::Deref,
        ]),
    };
    assert_eq!(place.ty(&ctx, &locals), i32_ty);
}

#[test]
fn chain_ref_to_array_index() {
    let (ctx, (ref_array, bool_ty)) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let bool_ty = c.bool_ty();
        let usize_ty = c.mk_ty(TyKind::Uint(UintTy::Usize));
        let array_ty = c.mk_ty(TyKind::Array(bool_ty, Const { kind: ConstKind::Uint(8), ty: usize_ty }));
        let ref_array = c.mk_ref(Region::Erased, array_ty, Mutability::Not);
        (ref_array, bool_ty)
    });

    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl { ty: ref_array, mutability: Mutability::Not, source_info: SourceInfo::new(Span::DUMMY) });

    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: Box::new([
            ProjectionElem::Deref,
            ProjectionElem::Index(LocalIdx::from_raw(1)),
        ]),
    };
    assert_eq!(place.ty(&ctx, &locals), bool_ty);
}

#[test]
fn chain_downcast_field_deref() {
    let (ctx, (tuple_ty, i32_ty)) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let i32_ty = c.mk_ty(TyKind::Int(IntTy::I32));
        let ref_i32 = c.mk_ref(Region::Erased, i32_ty, Mutability::Not);
        let substs = c.intern_substitution(vec![GenericArg::Ty(ref_i32)]);
        let tuple_ty = c.mk_ty(TyKind::Tuple(substs));
        (tuple_ty, i32_ty)
    });

    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl { ty: tuple_ty, mutability: Mutability::Not, source_info: SourceInfo::new(Span::DUMMY) });

    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: Box::new([
            ProjectionElem::Downcast(VariantIdx::from_raw(0)),
            ProjectionElem::Field(FieldIdx::from_raw(0)),
            ProjectionElem::Deref,
        ]),
    };
    assert_eq!(place.ty(&ctx, &locals), i32_ty);
}

#[test]
fn chain_error_propagation() {
    let (ctx, bool_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| c.bool_ty());

    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl { ty: bool_ty, mutability: Mutability::Not, source_info: SourceInfo::new(Span::DUMMY) });

    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: Box::new([
            ProjectionElem::Deref,
            ProjectionElem::Field(FieldIdx::from_raw(0)),
            ProjectionElem::Index(LocalIdx::from_raw(1)),
        ]),
    };

    assert_eq!(place.ty(&ctx, &locals), Ty::ERROR);
}

#[test]
fn chain_raw_ptr_to_slice_index() {
    let (ctx, (raw_ptr_slice, i32_ty)) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let i32_ty = c.mk_ty(TyKind::Int(IntTy::I32));
        let slice_ty = c.mk_ty(TyKind::Slice(i32_ty));
        let raw_ptr_slice = c.mk_ty(TyKind::RawPtr(slice_ty, Mutability::Not));
        (raw_ptr_slice, i32_ty)
    });

    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl { ty: raw_ptr_slice, mutability: Mutability::Not, source_info: SourceInfo::new(Span::DUMMY) });

    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: Box::new([
            ProjectionElem::Deref,
            ProjectionElem::Index(LocalIdx::from_raw(1)),
        ]),
    };
    assert_eq!(place.ty(&ctx, &locals), i32_ty);
}
