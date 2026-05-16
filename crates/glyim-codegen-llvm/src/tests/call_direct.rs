//! V14-T01: Call a defined function produces LLVM `call` instruction

use glyim_core::arena::IndexVec;
use glyim_core::{Abi, CrateId, DefId, Interner, LocalDefId, Mutability, Safety};
use glyim_mir::*;
use glyim_type::{FnSig, GenericArg, TyCtxMut, TyKind};

use crate::LlvmBackend;

fn make_simple_call_body(ctx: &mut TyCtxMut) -> Body {
    let i32_ty = ctx.mk_ty(TyKind::Int(glyim_core::IntTy::I32));
    let fn_sig = FnSig {
        inputs: ctx.intern_substitution(vec![GenericArg::Ty(i32_ty)]),
        output: i32_ty,
        c_variadic: false,
        unsafety: Safety::Safe,
        abi: Abi::Glyim,
    };
    let fn_ptr_ty = ctx.mk_ty(TyKind::FnPtr(fn_sig.clone()));

    let return_ty = i32_ty;
    let arg_count = 2;

    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl {
        ty: return_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: i32_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: fn_ptr_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });

    let const_42 = MirConst {
        kind: MirConstKind::Uint(42),
        ty: i32_ty,
        span: glyim_span::Span::DUMMY,
    };

    let bb0 = BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(2)),
                Rvalue::Use(Operand::Constant(const_42)),
            ),
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Call {
                func: Operand::Copy(Place::new(LocalIdx::from_raw(2))),
                args: vec![Operand::Copy(Place::new(LocalIdx::from_raw(1)))],
                destination: Place::new(LocalIdx::from_raw(0)),
                target: Some(BasicBlockIdx::from_raw(1)),
                cleanup: None,
            },
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        },
        is_cleanup: false,
    };

    let bb1 = BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        },
        is_cleanup: false,
    };

    Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        basic_blocks: IndexVec::from_raw(vec![bb0, bb1]),
        locals,
        arg_count,
        return_ty,
        span: glyim_span::Span::DUMMY,
        var_debug_info: vec![],
    }
}

#[test]
fn call_direct_produces_call_instruction() {
    let mut ctx_mut = TyCtxMut::new(Interner::default());
    let body = make_simple_call_body(&mut ctx_mut);
    let ctx = ctx_mut.freeze();

    let backend = LlvmBackend::new().with_ty_ctx(ctx);
    let inkwell_ctx = inkwell::context::Context::create();
    let result = backend.lower_body_to_module(&inkwell_ctx, &body);
    assert!(result.is_ok(), "lower_body_to_module failed: {:?}", result.err());

    let module = result.unwrap();
    let ir = module.print_to_string().to_string();
    assert!(ir.contains("call"), "LLVM IR must contain 'call' instruction:\n{}", ir);
}

#[test]
fn call_direct_has_function_with_body() {
    let mut ctx_mut = TyCtxMut::new(Interner::default());
    let body = make_simple_call_body(&mut ctx_mut);
    let ctx = ctx_mut.freeze();

    let backend = LlvmBackend::new().with_ty_ctx(ctx);
    let inkwell_ctx = inkwell::context::Context::create();
    let result = backend.lower_body_to_module(&inkwell_ctx, &body);
    assert!(result.is_ok());

    let module = result.unwrap();
    let func = module.get_first_function();
    assert!(func.is_some(), "Module must contain at least one function");
    let f = func.unwrap();
    assert!(f.count_basic_blocks() >= 1, "Function must have at least one basic block");
}
