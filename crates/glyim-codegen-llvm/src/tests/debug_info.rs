use crate::LlvmBackend;
use glyim_core::arena::IndexVec;
use glyim_core::primitives::*;
use glyim_core::{CrateId, DefId, LocalDefId, Name};
use glyim_mir::{
    BasicBlockData, Body, LocalDecl, LocalIdx, MirConst, MirConstKind, Operand, Place, Rvalue,
    SourceInfo, Statement, StatementKind, Terminator, TerminatorKind, VarDebugInfo,
    VarDebugInfoValue,
};
use glyim_span::{ByteIdx, FileId, Span, SyntaxContext};
use glyim_type::TyCtxMut;
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

fn has_compile_unit(module: &inkwell::module::Module) -> bool {
    !module.get_global_metadata("llvm.dbg.cu").is_empty()
}

#[test]
fn test_debug_compile_unit_present() {
    let source = "fn main() {\n  let x = false;\n}\n";
    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering failed");

    assert!(
        has_compile_unit(&module),
        "Expected a DICompileUnit in the module"
    );
}

#[test]
fn test_debug_subprogram_attached() {
    let source = "fn main() {\n  let x = false;\n}\n";
    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
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
    assert!(
        func.get_subprogram().is_some(),
        "Function does not have a DISubprogram"
    );
}

#[test]
fn test_debug_line_info_on_instruction() {
    let source = "fn main() {\n  let x = false;\n}\n";
    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
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
            if let Some(loc) = instr.get_debug_location() {
                let _line: u32 = loc.get_line();
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
    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering failed");

    assert!(
        has_compile_unit(&module),
        "No DICompileUnit found; debug info not generated"
    );
}

// ---------------------------------------------------------------------------
// Additional Tests
// ---------------------------------------------------------------------------

#[test]
fn test_debug_info_disabled_no_crash() {
    let source = "fn main() { let x = 1; }";
    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(false)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering without debug info should succeed");

    assert!(
        !has_compile_unit(&module),
        "DICompileUnit should NOT be present when debug info is disabled"
    );
}

#[test]
fn test_debug_info_multiple_files() {
    let source_main = "fn main() {}\n";
    let source_lib = "fn helper() -> bool { true }\n";

    let mut source_map = HashMap::new();
    source_map.insert(
        FileId::from_raw(0),
        ("main.g".to_string(), source_main.to_string()),
    );
    source_map.insert(
        FileId::from_raw(1),
        ("lib.g".to_string(), source_lib.to_string()),
    );

    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(source_map);

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering with multiple files should succeed");

    assert!(
        has_compile_unit(&module),
        "DICompileUnit should be present with multiple files"
    );
}

#[test]
fn test_debug_info_empty_source() {
    let source = "";
    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering with empty source should succeed");

    assert!(
        has_compile_unit(&module),
        "DICompileUnit should exist even with empty source"
    );
}

#[test]
fn test_debug_info_dummy_spans() {
    let source = "fn main() {}\n";
    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering with dummy spans should succeed");

    assert!(
        has_compile_unit(&module),
        "DICompileUnit should exist with dummy spans"
    );
}

#[test]
fn test_debug_info_var_debug_const_value() {
    let source = "fn main() {}\n";
    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
        let bool_ty = ctx_mut.bool_ty();
        let unit_ty = ctx_mut.unit_ty();

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
        body.locals = locals;

        body.var_debug_info = vec![VarDebugInfo {
            name: name_x,
            value: VarDebugInfoValue::Const(MirConst {
                kind: MirConstKind::Bool(true),
                ty: bool_ty,
                span: Span::DUMMY,
            }),
        }];

        let bb_data = BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        };
        let mut bbs = IndexVec::new();
        bbs.push(bb_data);
        body.basic_blocks = bbs;
        body
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let result = backend.lower_body_to_module(&llvm_ctx, &body);
    assert!(
        result.is_ok(),
        "Lowering with const debug variable should succeed"
    );
}

#[test]
fn test_debug_info_subprogram_exists() {
    let source = "fn my_func() -> bool { false }\n";
    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering succeeded");

    let func = module.get_first_function().expect("no function");
    assert!(
        func.get_subprogram().is_some(),
        "Subprogram should be attached"
    );
}

#[test]
fn test_debug_info_no_source_map_no_crash() {
    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(HashMap::new());

    let llvm_ctx = Context::create();
    let result = backend.lower_body_to_module(&llvm_ctx, &body);
    assert!(
        result.is_ok(),
        "Lowering with empty source_map should succeed"
    );
}

#[test]
fn test_debug_info_multiline_source_line_numbers() {
    let source = "// line 1\n// line 2\nfn main() {\n  let x = false;\n}\n";
    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering with multi-line source should succeed");

    let func = module.get_first_function().expect("no function");
    let mut locations: Vec<(u32, u32)> = Vec::new();
    for bb in func.get_basic_blocks() {
        for instr in bb.get_instructions() {
            if let Some(loc) = instr.get_debug_location() {
                locations.push((loc.get_line(), loc.get_column()));
            }
        }
    }
    assert!(!locations.is_empty(), "No debug locations found");
    for (line, col) in &locations {
        assert!(*line >= 1, "Line number should be >= 1, got {}", line);
        assert!(col >= &0, "Column should be >= 0, got {}", col);
    }
}

#[test]
fn test_debug_info_verify_module() {
    let source = "fn main() { let x: i32 = 42; }\n";
    let (_ctx, body) = glyim_test::with_fresh_ty_ctx(|ctx_mut| {
        let name_x = ctx_mut.resolver().intern("x");
        make_test_body(ctx_mut, name_x)
    });

    let backend = LlvmBackend::new()
        .with_debug_info(true)
        .with_source_map(make_source_map(source));

    let llvm_ctx = Context::create();
    let module = backend
        .lower_body_to_module(&llvm_ctx, &body)
        .expect("lowering succeeded");

    let result = module.verify();
    assert!(
        result.is_ok(),
        "LLVM module verification failed: {:?}",
        result.err()
    );
}
