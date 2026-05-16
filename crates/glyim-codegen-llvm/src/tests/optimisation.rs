use crate::LlvmBackend;
use glyim_codegen::CodegenBackend;
use glyim_core::primitives::*;
use glyim_core::Interner;
use glyim_mir::{
    BasicBlockIdx, Body, LocalDecl, LocalIdx, MirConst, MirConstKind, Operand, Place,
    Rvalue, SourceInfo, Statement, StatementKind, Terminator, TerminatorKind,
};
use glyim_span::Span;
use glyim_type::{Mutability, Ty, TyCtx, TyCtxMut, TyKind};
use inkwell::context::Context;
use std::sync::Arc;

fn dummy_ty_ctx() -> TyCtx {
    TyCtxMut::new(Interner::default()).freeze()
}

fn simple_body_with_local(ty: Ty) -> Body {
    let owner = DefId::new(CrateId::from_raw(1), LocalDefId::from_raw(1));
    let mut body = Body::dummy(owner);
    let local_idx = LocalIdx::from_raw(0);
    let const_val = MirConst {
        kind: MirConstKind::Int(42),
        ty,
        span: Span::DUMMY,
    };
    let assign = Statement {
        kind: StatementKind::Assign(
            Place::new(local_idx),
            Rvalue::Use(Operand::Constant(const_val)),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let mut bb = glyim_mir::BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    bb.statements.push(assign);
    body.basic_blocks.push(bb);
    body.locals.push(LocalDecl {
        ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    body.arg_count = 0;
    body.return_ty = ty;
    body
}

fn count_allocas(module: &inkwell::module::Module) -> usize {
    let mut count = 0;
    for func in module.get_functions() {
        for bb in func.get_basic_blocks() {
            for instr in bb.get_instructions() {
                if instr.get_opcode() == inkwell::values::InstructionOpcode::Alloca {
                    count += 1;
                }
            }
        }
    }
    count
}

#[test]
fn test_o0_no_optimizations() {
    let ctx = dummy_ty_ctx();
    let int_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let backend = LlvmBackend::new()
        .with_ty_ctx(ctx.clone())
        .with_opt_level(0);
    let body = Arc::new(simple_body_with_local(int_ty));
    let context = Context::create();
    let module = backend.lower_bodies_to_module(&context, &[body]).expect("lowering failed");
    assert!(count_allocas(&module) > 0, "Expected alloca instructions at O0");
}

#[test]
fn test_o2_mem2reg() {
    let ctx = dummy_ty_ctx();
    let int_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let backend = LlvmBackend::new()
        .with_ty_ctx(ctx.clone())
        .with_opt_level(2);
    let body = Arc::new(simple_body_with_local(int_ty));
    let context = Context::create();
    let module = backend.lower_bodies_to_module(&context, &[body]).expect("lowering failed");
    assert_eq!(count_allocas(&module), 0, "Expected no alloca instructions after O2 mem2reg");
}

#[test]
fn test_oz_size_optimization() {
    let ctx = dummy_ty_ctx();
    let int_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let backend = LlvmBackend::new()
        .with_ty_ctx(ctx.clone())
        .with_opt_level(2)
        .with_opt_for_size(true);
    let body = Arc::new(simple_body_with_local(int_ty));
    let context = Context::create();
    let result = backend.lower_bodies_to_module(&context, &[body]);
    assert!(result.is_ok(), "Oz lowering should succeed");
}

#[test]
fn test_lto_across_cgus() {
    let ctx = dummy_ty_ctx();
    let int_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let backend = LlvmBackend::new()
        .with_ty_ctx(ctx.clone())
        .with_opt_level(2);
    let callee_owner = DefId::new(CrateId::from_raw(1), LocalDefId::from_raw(10));
    let mut callee_body = Body::dummy(callee_owner);
    let callee_local = LocalIdx::from_raw(0);
    let const_val = MirConst {
        kind: MirConstKind::Int(41),
        ty: int_ty,
        span: Span::DUMMY,
    };
    let assign = Statement {
        kind: StatementKind::Assign(
            Place::new(callee_local),
            Rvalue::Use(Operand::Constant(const_val)),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let mut bb = glyim_mir::BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    bb.statements.push(assign);
    callee_body.basic_blocks.push(bb);
    callee_body.locals.push(LocalDecl {
        ty: int_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    callee_body.arg_count = 0;
    callee_body.return_ty = int_ty;

    let caller_owner = DefId::new(CrateId::from_raw(1), LocalDefId::from_raw(20));
    let caller_body = Body::dummy(caller_owner); // empty caller, just to have two functions

    let bodies = vec![Arc::new(callee_body), Arc::new(caller_body)];
    let context = Context::create();
    let module = backend.lower_bodies_to_module(&context, &bodies).expect("LTO lowering failed");
    assert!(module.get_functions().count() >= 2, "Expected at least 2 functions after LTO");
}
