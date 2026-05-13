use crate::*;
use glyim_core::arena::IndexVec;
use glyim_core::primitives::{IntTy, Mutability, UintTy};
use glyim_span::Span;
use glyim_type::{
    Const, ConstKind, FieldIdx, GenericArg, Region, Ty, TyCtxMut, TyKind, VariantIdx,
};
use glyim_test::with_fresh_ty_ctx;

/// Property: Place::ty() with no projection always returns the local's declared type
#[test]
fn property_no_projection_returns_local_type() {
    let (ctx, types) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let bool_ty = c.bool_ty();
        let unit_ty = c.unit_ty();
        let never_ty = c.never_ty();
        let i32_ty = c.mk_ty(TyKind::Int(IntTy::I32));
        let u64_ty = c.mk_ty(TyKind::Uint(UintTy::U64));
        let f64_ty = c.mk_ty(TyKind::Float(glyim_type::FloatTy::F64));
        let string_ty = c.mk_ty(TyKind::String);
        let char_ty = c.mk_ty(TyKind::Char);
        (vec![bool_ty, unit_ty, never_ty, i32_ty, u64_ty, f64_ty, string_ty, char_ty])
    });

    for (i, expected_ty) in types.iter().enumerate() {
        let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
        locals.push(LocalDecl {
            ty: Ty::ERROR,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });
        locals.push(LocalDecl {
            ty: *expected_ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        let place = Place::new(LocalIdx::from_raw(1));
        assert_eq!(place.ty(&ctx, &locals), *expected_ty,
            "Property failed for type at index {}", i);
    }
}

/// Property: Deref on non-pointer types always returns ERROR
#[test]
fn property_deref_on_non_pointer_always_error() {
    let (ctx, types) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let bool_ty = c.bool_ty();
        let unit_ty = c.unit_ty();
        let i32_ty = c.mk_ty(TyKind::Int(IntTy::I32));
        let string_ty = c.mk_ty(TyKind::String);
        let char_ty = c.mk_ty(TyKind::Char);
        let slice_ty = c.mk_ty(TyKind::Slice(bool_ty));
        let usize_ty = c.mk_ty(TyKind::Uint(UintTy::Usize));
        let array_ty = c.mk_ty(TyKind::Array(bool_ty, Const { kind: ConstKind::Uint(3), ty: usize_ty }));
        (vec![bool_ty, unit_ty, i32_ty, string_ty, char_ty, slice_ty, array_ty])
    });

    for (i, ty) in types.iter().enumerate() {
        let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
        locals.push(LocalDecl {
            ty: *ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        let place = Place {
            local: LocalIdx::from_raw(0),
            projection: Box::new([ProjectionElem::Deref]),
        };
        assert_eq!(place.ty(&ctx, &locals), Ty::ERROR,
            "Property failed: Deref on non-pointer type {} should return ERROR", i);
    }
}

/// Property: Downcast always returns the same type regardless of variant index
#[test]
fn property_downcast_preserves_type() {
    let (ctx, tuple_ty) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let i32_ty = c.mk_ty(TyKind::Int(IntTy::I32));
        let substs = c.intern_substitution(vec![GenericArg::Ty(i32_ty)]);
        c.mk_ty(TyKind::Tuple(substs))
    });

    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl {
        ty: tuple_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });

    for variant in 0..5u32 {
        let place = Place {
            local: LocalIdx::from_raw(0),
            projection: Box::new([ProjectionElem::Downcast(VariantIdx::from_raw(variant))]),
        };
        assert_eq!(place.ty(&ctx, &locals), tuple_ty,
            "Property failed: Downcast with variant {} should preserve type", variant);
    }
}

/// Property: Field on non-tuple/non-ADT always returns ERROR
#[test]
fn property_field_on_non_tuple_always_error() {
    let (ctx, types) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let bool_ty = c.bool_ty();
        let i32_ty = c.mk_ty(TyKind::Int(IntTy::I32));
        let string_ty = c.mk_ty(TyKind::String);
        let ref_bool = c.mk_ref(Region::Erased, bool_ty, Mutability::Not);
        let raw_ptr = c.mk_ty(TyKind::RawPtr(bool_ty, Mutability::Not));
        let slice_ty = c.mk_ty(TyKind::Slice(i32_ty));
        (vec![bool_ty, i32_ty, string_ty, ref_bool, raw_ptr, slice_ty])
    });

    for (i, ty) in types.iter().enumerate() {
        let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
        locals.push(LocalDecl {
            ty: *ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        for field_idx in 0..3u32 {
            let place = Place {
                local: LocalIdx::from_raw(0),
                projection: Box::new([ProjectionElem::Field(FieldIdx::from_raw(field_idx))]),
            };
            assert_eq!(place.ty(&ctx, &locals), Ty::ERROR,
                "Property failed: Field({}) on non-tuple type {} should return ERROR", field_idx, i);
        }
    }
}

/// Property: Index on array/slice always returns element type
#[test]
fn property_index_returns_element_type() {
    let (ctx, (array_ty, slice_ty, i32_ty)) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let i32_ty = c.mk_ty(TyKind::Int(IntTy::I32));
        let usize_ty = c.mk_ty(TyKind::Uint(UintTy::Usize));
        let array_ty = c.mk_ty(TyKind::Array(i32_ty, Const { kind: ConstKind::Uint(10), ty: usize_ty }));
        let slice_ty = c.mk_ty(TyKind::Slice(i32_ty));
        (array_ty, slice_ty, i32_ty)
    });

    for (i, container_ty) in [array_ty, slice_ty].iter().enumerate() {
        let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
        locals.push(LocalDecl {
            ty: *container_ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        for idx_local in 0..3u32 {
            let place = Place {
                local: LocalIdx::from_raw(0),
                projection: Box::new([ProjectionElem::Index(LocalIdx::from_raw(idx_local))]),
            };
            assert_eq!(place.ty(&ctx, &locals), i32_ty,
                "Property failed: Index on container {} with idx_local {} should return element type", i, idx_local);
        }
    }
}

/// Property: Deref on &T and &mut T both return T
#[test]
fn property_deref_shared_and_mut_ref_same_inner() {
    let (ctx, (ref_bool, ref_mut_bool, bool_ty)) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let bool_ty = c.bool_ty();
        let ref_bool = c.mk_ref(Region::Erased, bool_ty, Mutability::Not);
        let ref_mut_bool = c.mk_ref(Region::Erased, bool_ty, Mutability::Mut);
        (ref_bool, ref_mut_bool, bool_ty)
    });

    for (i, ref_ty) in [ref_bool, ref_mut_bool].iter().enumerate() {
        let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
        locals.push(LocalDecl {
            ty: *ref_ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        let place = Place {
            local: LocalIdx::from_raw(0),
            projection: Box::new([ProjectionElem::Deref]),
        };
        assert_eq!(place.ty(&ctx, &locals), bool_ty,
            "Property failed: Deref on ref variant {} should return inner type", i);
    }
}

/// Property: Deref on *const T and *mut T both return T
#[test]
fn property_deref_raw_ptr_shared_and_mut_same_inner() {
    let (ctx, (raw_const, raw_mut, bool_ty)) = with_fresh_ty_ctx(|c: &mut TyCtxMut| {
        let bool_ty = c.bool_ty();
        let raw_const = c.mk_ty(TyKind::RawPtr(bool_ty, Mutability::Not));
        let raw_mut = c.mk_ty(TyKind::RawPtr(bool_ty, Mutability::Mut));
        (raw_const, raw_mut, bool_ty)
    });

    for (i, ptr_ty) in [raw_const, raw_mut].iter().enumerate() {
        let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
        locals.push(LocalDecl {
            ty: *ptr_ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(Span::DUMMY),
        });

        let place = Place {
            local: LocalIdx::from_raw(0),
            projection: Box::new([ProjectionElem::Deref]),
        };
        assert_eq!(place.ty(&ctx, &locals), bool_ty,
            "Property failed: Deref on raw ptr variant {} should return inner type", i);
    }
}
