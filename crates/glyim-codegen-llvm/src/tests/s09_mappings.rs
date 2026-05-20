use crate::types::llvm_type_for_ty;
use glyim_core::Interner;
use glyim_core::primitives::TargetInfo;
use glyim_type::{Ty, TyCtxMut};
use inkwell::context::Context;

#[test]
fn s09_t01_bool_emits_as_int() {
    let ctx_mut = TyCtxMut::new(Interner::default());
    let ctx = ctx_mut.freeze();
    let target_info = TargetInfo::default();
    let context = Context::create();
    // Using Ty::BOOL sentinel - guaranteed to exist and be a valid Ty
    let llvm_ty = llvm_type_for_ty(&ctx, &target_info, &context, Ty::BOOL);
    assert!(llvm_ty.is_int_type());
}

#[test]
fn s09_t02_unit_emits_as_struct() {
    let ctx_mut = TyCtxMut::new(Interner::default());
    let ctx = ctx_mut.freeze();
    let target_info = TargetInfo::default();
    let context = Context::create();
    // Using Ty::UNIT sentinel
    let llvm_ty = llvm_type_for_ty(&ctx, &target_info, &context, Ty::UNIT);
    assert!(llvm_ty.is_struct_type());
}
