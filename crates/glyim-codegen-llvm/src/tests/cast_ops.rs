use super::helpers::*;
use glyim_core::primitives::*;
use glyim_mir::Rvalue;
use glyim_type::{TyCtxMut, TyKind};

#[test]
fn test_int_to_int_trunc() {
    let mut ctx_mut = TyCtxMut::new(glyim_core::Interner::default());
    let i64_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I64));
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let frozen = ctx_mut.freeze();
    let op = const_operand_i32(0x12345678, i64_ty); // 305419896 as i64
    let rv = Rvalue::Cast(glyim_mir::CastKind::IntToInt, op, i32_ty);
    let body = simple_mir_body(i32_ty, rv);
    let backend = crate::LlvmBackend::new().with_ty_ctx(frozen);
    let context = inkwell::context::Context::create();
    let module = backend.lower_body_to_module(&context, &body).expect("lowering");
    let ir = module.print_to_string().to_string();
    // LLVM constant-folds trunc(i64 305419896 to i32) -> store i32 305419896
    assert!(
        ir.contains("store i32 305419896"),
        "Expected 'store i32 305419896' in IR:\n{}",
        ir
    );
}

#[test]
fn test_int_to_float() {
    // Placeholder: currently stub
}
