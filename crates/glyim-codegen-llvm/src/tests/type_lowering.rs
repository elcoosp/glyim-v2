//! S22-T01: Tests for LLVM type lowering (llvm_type_for_ty).

use glyim_core::primitives::*;
use glyim_test::{test_frozen_ty_ctx, with_fresh_ty_ctx};
use glyim_type::*;
use inkwell::context::Context;

#[test]
fn error_type_maps_to_i64() {
    let ctx = test_frozen_ty_ctx();
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, Ty::ERROR);
    assert!(llvm_ty.is_int_type(), "Error type should map to int type");
    assert_eq!(llvm_ty.into_int_type().get_bit_width(), 64);
}

#[test]
fn never_type_maps_to_empty_struct() {
    let ctx = test_frozen_ty_ctx();
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, Ty::NEVER);
    assert!(
        llvm_ty.is_struct_type(),
        "Never type should map to struct type"
    );
    assert_eq!(llvm_ty.into_struct_type().get_field_types().len(), 0);
}

#[test]
fn unit_type_maps_to_empty_struct() {
    let ctx = test_frozen_ty_ctx();
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, Ty::UNIT);
    assert!(
        llvm_ty.is_struct_type(),
        "Unit type should map to struct type"
    );
    assert_eq!(llvm_ty.into_struct_type().get_field_types().len(), 0);
}

#[test]
fn bool_type_maps_to_i1() {
    let ctx = test_frozen_ty_ctx();
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, Ty::BOOL);
    assert!(llvm_ty.is_int_type());
    assert_eq!(llvm_ty.into_int_type().get_bit_width(), 1);
}

#[test]
fn i32_type_maps_to_i32() {
    let (ctx, i32_ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I32)));
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, i32_ty);
    assert!(llvm_ty.is_int_type());
    assert_eq!(llvm_ty.into_int_type().get_bit_width(), 32);
}

#[test]
fn u8_type_maps_to_i8() {
    let (ctx, u8_ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Uint(UintTy::U8)));
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, u8_ty);
    assert!(llvm_ty.is_int_type());
    assert_eq!(llvm_ty.into_int_type().get_bit_width(), 8);
}

#[test]
fn float_f32_maps_to_f32() {
    let (ctx, f32_ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Float(FloatTy::F32)));
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, f32_ty);
    assert!(llvm_ty.is_float_type(), "F32 type should map to float type");
}

#[test]
fn float_f64_maps_to_f64() {
    let (ctx, f64_ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Float(FloatTy::F64)));
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, f64_ty);
    assert!(llvm_ty.is_float_type(), "F64 type should map to float type");
}

#[test]
fn char_type_maps_to_i32() {
    let (ctx, char_ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Char));
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, char_ty);
    assert!(llvm_ty.is_int_type());
    assert_eq!(llvm_ty.into_int_type().get_bit_width(), 32);
}

#[test]
fn string_type_maps_to_ptr() {
    let (ctx, string_ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::String));
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, string_ty);
    assert!(
        llvm_ty.is_struct_type(),
        "String type should map to fat pointer struct"
    );
}

#[test]
fn ref_type_maps_to_ptr() {
    let (ctx, ref_ty) = with_fresh_ty_ctx(|c| {
        let inner = c.bool_ty();
        c.mk_ref(Region::Erased, inner, Mutability::Not)
    });
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, ref_ty);
    assert!(llvm_ty.is_pointer_type());
}

#[test]
fn raw_ptr_type_maps_to_ptr() {
    let (ctx, ptr_ty) = with_fresh_ty_ctx(|c| {
        let inner = c.bool_ty();
        c.mk_ty(TyKind::RawPtr(inner, Mutability::Not))
    });
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, ptr_ty);
    assert!(llvm_ty.is_pointer_type());
}

#[test]
fn fn_ptr_type_maps_to_ptr() {
    let (ctx, fn_ptr_ty) = with_fresh_ty_ctx(|c| {
        let ret = c.bool_ty();
        let inputs = c.intern_substitution(vec![]);
        let sig = FnSig {
            inputs,
            output: ret,
            c_variadic: false,
            unsafety: Safety::Safe,
            abi: Abi::Glyim,
        };
        c.mk_fn_ptr(sig)
    });
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, fn_ptr_ty);
    assert!(llvm_ty.is_pointer_type());
}

#[test]
fn tuple_type_maps_to_struct() {
    let (ctx, tuple_ty) = with_fresh_ty_ctx(|c| {
        let i32_ty = c.mk_ty(TyKind::Int(IntTy::I32));
        let bool_ty = c.bool_ty();
        let subst = c.intern_substitution(vec![GenericArg::Ty(i32_ty), GenericArg::Ty(bool_ty)]);
        c.mk_tuple(subst)
    });
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, tuple_ty);
    assert!(llvm_ty.is_struct_type());
    let st = llvm_ty.into_struct_type();
    assert_eq!(st.get_field_types().len(), 2);
}

#[test]
fn empty_tuple_maps_to_empty_struct() {
    let (ctx, et) = with_fresh_ty_ctx(|c| {
        let subst = c.intern_substitution(vec![]);
        c.mk_tuple(subst)
    });
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, et);
    assert!(llvm_ty.is_struct_type());
    assert_eq!(llvm_ty.into_struct_type().get_field_types().len(), 0);
}

#[test]
fn array_type_maps_to_llvm_array() {
    let (ctx, array_ty) = with_fresh_ty_ctx(|c| {
        let i32_ty = c.mk_ty(TyKind::Int(IntTy::I32));
        let count = Const {
            kind: ConstKind::Uint(4),
            ty: c.mk_ty(TyKind::Uint(UintTy::Usize)),
        };
        c.mk_ty(TyKind::Array(i32_ty, count))
    });
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, array_ty);
    assert!(
        llvm_ty.is_array_type(),
        "Array type should map to LLVM array type"
    );
}

#[test]
fn slice_type_maps_to_fat_ptr() {
    let (ctx, slice_ty) = with_fresh_ty_ctx(|c| {
        let i32_ty = c.mk_ty(TyKind::Int(IntTy::I32));
        c.mk_ty(TyKind::Slice(i32_ty))
    });
    let context = Context::create();
    let target_info = TargetInfo::default();
    let llvm_ty = crate::types::llvm_type_for_ty(&ctx, &target_info, &context, slice_ty);
    assert!(
        llvm_ty.is_struct_type(),
        "Slice should map to struct (fat pointer)"
    );
    assert_eq!(llvm_ty.into_struct_type().get_field_types().len(), 2);
}
