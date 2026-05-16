use glyim_codegen_llvm::LlvmBackend;
use glyim_core::arena::IndexVec;
use glyim_core::primitives::*;
use glyim_core::{DefId, Name};
use glyim_mir::{
    BasicBlockData, Body, LocalDecl, LocalIdx, MirConst, MirConstKind, Operand, Place, Rvalue,
    SourceInfo, Statement, StatementKind, Terminator, TerminatorKind, VarDebugInfo,
    VarDebugInfoValue,
};
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use glyim_type::{TyCtx, TyCtxMut};
use inkwell::context::Context;
use std::collections::HashMap;

fn make_test_body(ctx: &TyCtxMut, var_name: Name) -> Body {
    let bool_ty = ctx.bool_ty();
    let unit_ty = ctx.unit_ty();

    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    let mut locals: IndexVec<LocalIdx, LocalDecl> = IndexVec::new();
    locals.push(LocalDecl {
        ty: unit_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::new(
            FileId::from_raw(0),
            ByteIdx::from_raw(0),
            ByteIdx::from_raw(0),
            SyntaxContext::ROOT,
        )),
    });
    locals.push(LocalDecl {
        ty: bool_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::new(
            FileId::from_raw(0),
            ByteIdx::from_raw(10),
            ByteIdx::from_raw(15),
            SyntaxContext::ROOT,
        )),
    });
    body.locals = locals;

    body.var_debug_info = vec![VarDebugInfo {
        name: var_name,
        value: VarDebugInfoValue::Place(Place::new(LocalIdx::from_raw(1))),
    }];

    let stmts = vec![Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(1)),
            Rvalue::Use(Operand::Constant(MirConst {
                kind: MirConstKind::Bool(false),
                ty: bool_ty,
                span: Span::new(
                    FileId::from_raw(0),
                    ByteIdx::from_raw(12),
                    ByteIdx::from_raw(17),
                    SyntaxContext::ROOT,
                ),
            })),
        ),
        source_info: SourceInfo::new(Span::new(
            FileId::from_raw(0),
            ByteIdx::from_raw(10),
            ByteIdx::from_raw(17),
            SyntaxContext::ROOT,
        )),
    }];

    let bb_data = BasicBlockData {
        statements: stmts,
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::new(
                FileId::from_raw(0),
                ByteIdx::from_raw(18),
                ByteIdx::from_raw(19),
                SyntaxContext::ROOT,
            )),
        },
        is_cleanup: false,
    };

    let mut bbs = IndexVec::new();
    bbs.push(bb_data);
    body.basic_blocks = bbs;
    body
}

fn make_source_map(source: &str) -> HashMap<FileId, (String, String)> {
    let mut map = HashMap::new();
    map.insert(
        FileId::from_raw(0),
        ("test.g".to_string(), source.to_string()),
    );
    map
}

fn get_compile_unit(
    module: &inkwell::module::Module,
) -> Option<inkwell::debug_info::DICompileUnit> {
    let metadata = module.get_named_metadata("llvm.dbg.cu")?;
    let operands = metadata.get_operands();
    operands
        .get(0)?
        .as_metadata_node()
        .map(|node| unsafe { inkwell::debug_info::DICompileUnit::from_raw(node.as_ptr()) })
}

#[test]
fn test_debug_compile_unit_present() {
    let source = "fn main() {\n  let x = false;\n}\n";
    let (ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver.intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering failed");

    let compile_unit = get_compile_unit(&module);
    assert!(compile_unit.is_some(), "Expected a DICompileUnit in the module");
}

#[test]
fn test_debug_subprogram_attached() {
    let source = "fn main() {\n  let x = false;\n}\n";
    let (ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver.intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering failed");

    let func = module.get_first_function().expect("no function in module");
    let subprogram = func
        .get_subprogram()
        .expect("Function does not have a DISubprogram");
    assert!(
        !subprogram.get_name().to_string().is_empty(),
        "Subprogram name is empty"
    );
}

#[test]
fn test_debug_line_info_on_instruction() {
    let source = "fn main() {\n  let x = false;\n}\n";
    let (ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver.intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering failed");

    let func = module.get_first_function().expect("no function in module");
    let mut has_location = false;
    for bb in func.get_basic_blocks() {
        for instr in bb.get_instructions() {
            if let Some(loc) = instr.get_debug_loc() {
                let line = loc.get_line();
                assert!(line > 0, "DILocation line should be > 0");
                has_location = true;
                break;
            }
        }
    }
    assert!(has_location, "No instruction with DILocation found");
}

#[test]
fn test_debug_local_variable_has_di() {
    let source = "fn main() {\n  let x = false;\n}\n";
    let (ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver.intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering failed");

    let func = module.get_first_function().expect("no function");
    let mut seen_declare = false;
    for bb in func.get_basic_blocks() {
        for instr in bb.get_instructions() {
            if instr.get_opcode() == inkwell::values::InstructionOpcode::Call {
                if let Some(call) = instr.get_instruction_call() {
                    if let Some(called_val) = call.get_called_value() {
                        let name = called_val.get_name().to_str().unwrap_or("");
                        if name == "llvm.dbg.declare" {
                            seen_declare = true;
                            break;
                        }
                    }
                }
            }
        }
    }
    assert!(seen_declare, "No llvm.dbg.declare intrinsic found");
}
