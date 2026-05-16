use super::helpers::*;
use glyim_core::primitives::*;
use glyim_mir::{MirConst, MirConstKind, Operand, Rvalue};
use glyim_span::Span;
use glyim_type::{TyCtxMut, TyKind};

#[test]
fn test_int_to_int_trunc() {
    let mut ctx_mut = TyCtxMut::new(glyim_core::Interner::default());
    let i64_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I64));
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let frozen = ctx_mut.freeze();
    let op = const_operand_i32(0x12345678, i64_ty);
    let rv = Rvalue::Cast(glyim_mir::CastKind::IntToInt, op, i32_ty);
    let body = simple_mir_body(i32_ty, rv);
    let backend = crate::LlvmBackend::new().with_ty_ctx(frozen);
    let context = inkwell::context::Context::create();
    let module = backend
        .lower_body_to_module(&context, &body)
        .expect("lowering");
    let ir = module.print_to_string().to_string();
    assert!(
        ir.contains("store i32 305419896"),
        "Expected 'store i32 305419896' in IR:\n{}",
        ir
    );
}

#[test]
fn test_int_to_int_extend() {
    let mut ctx_mut = TyCtxMut::new(glyim_core::Interner::default());
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let i64_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I64));
    let frozen = ctx_mut.freeze();
    let op = const_operand_i32(42, i32_ty);
    let rv = Rvalue::Cast(glyim_mir::CastKind::IntToInt, op, i64_ty);
    let body = simple_mir_body(i64_ty, rv);
    let backend = crate::LlvmBackend::new().with_ty_ctx(frozen);
    let context = inkwell::context::Context::create();
    let module = backend
        .lower_body_to_module(&context, &body)
        .expect("lowering");
    let ir = module.print_to_string().to_string();
    assert!(
        ir.contains("store i64 42"),
        "Expected 'store i64 42' in IR:\n{}",
        ir
    );
}

#[test]
fn test_int_to_float() {
    let mut ctx_mut = TyCtxMut::new(glyim_core::Interner::default());
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let f64_ty = ctx_mut.mk_ty(TyKind::Float(FloatTy::F64));
    let frozen = ctx_mut.freeze();
    let op = const_operand_i32(42, i32_ty);
    let rv = Rvalue::Cast(glyim_mir::CastKind::IntToFloat, op, f64_ty);
    let body = simple_mir_body(f64_ty, rv);
    let backend = crate::LlvmBackend::new().with_ty_ctx(frozen);
    let context = inkwell::context::Context::create();
    let module = backend
        .lower_body_to_module(&context, &body)
        .expect("lowering");
    let ir = module.print_to_string().to_string();
    assert!(
        ir.contains("store double") || ir.contains("sitofp"),
        "Expected float store or sitofp in IR:\n{}",
        ir
    );
}

#[test]
fn test_float_to_int() {
    let mut ctx_mut = TyCtxMut::new(glyim_core::Interner::default());
    let f64_ty = ctx_mut.mk_ty(TyKind::Float(FloatTy::F64));
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let frozen = ctx_mut.freeze();
    let op = Operand::Constant(MirConst {
        kind: MirConstKind::FloatBits((42.0f64).to_bits()),
        ty: f64_ty,
        span: Span::DUMMY,
    });
    let rv = Rvalue::Cast(glyim_mir::CastKind::FloatToInt, op, i32_ty);
    let body = simple_mir_body(i32_ty, rv);
    let backend = crate::LlvmBackend::new().with_ty_ctx(frozen);
    let context = inkwell::context::Context::create();
    let module = backend
        .lower_body_to_module(&context, &body)
        .expect("lowering");
    let ir = module.print_to_string().to_string();
    assert!(
        ir.contains("store i32 42"),
        "Expected store i32 42 in IR:
{}",
        ir
    );
}
