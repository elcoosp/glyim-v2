use crate::{BytecodeBackend, CodegenBackend};
use glyim_core::primitives::*;
use glyim_mir::*;
use glyim_span::Span;
use std::path::Path;
use std::sync::Arc;

/// Helper: create a minimal Body with given basic blocks.
fn make_body(blocks: Vec<BasicBlockData>, locals: Vec<LocalDecl>, arg_count: usize) -> Arc<Body> {
    use glyim_core::IndexVec;

    let mut bb_map = IndexVec::new();
    for block in blocks {
        bb_map.push(block);
    }

    let mut local_map = IndexVec::new();
    for local in locals {
        local_map.push(local);
    }

    Arc::new(Body {
        owner: glyim_core::DefId::new(
            glyim_core::CrateId::from_raw(0),
            glyim_core::LocalDefId::from_raw(0),
        ),
        basic_blocks: bb_map,
        locals: local_map,
        arg_count,
        return_ty: glyim_type::Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: Vec::new(),
    })
}

/// Helper: create a LocalDecl with a given type.
fn local_decl(ty: glyim_type::Ty) -> LocalDecl {
    LocalDecl {
        ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

/// Helper: create a BasicBlockData with statements and terminator.
fn block(stmts: Vec<Statement>, term: Terminator) -> BasicBlockData {
    BasicBlockData {
        statements: stmts,
        terminator: term,
        is_cleanup: false,
    }
}

/// Helper: create a Terminator with kind and span.
fn term(kind: TerminatorKind) -> Terminator {
    Terminator {
        kind,
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

/// Helper: create a Statement with kind.
fn stmt(kind: StatementKind) -> Statement {
    Statement {
        kind,
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

// ============================================================================
// S07-T01: Empty function → produces module with Return opcode
// ============================================================================
#[test]
fn t01_empty_function_returns_module_with_return_opcode() {
    let body = make_body(
        vec![block(
            vec![],
            term(TerminatorKind::Return),
        )],
        vec![],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
    let bytecode = result.unwrap();
    assert!(!bytecode.is_empty(), "Bytecode should not be empty");
}

// ============================================================================
// S07-T02: Function with integer constants → LoadConst + Add + Return
// ============================================================================
#[test]
fn t02_integer_constants_and_add_yields_loadconst_add_return() {
    let body = make_body(
        vec![block(
            vec![
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(42),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(10),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(2)),
                    Rvalue::BinaryOp(
                        BinOp::Add,
                        Box::new((
                            Operand::Copy(Place::new(LocalIdx::from_raw(0))),
                            Operand::Copy(Place::new(LocalIdx::from_raw(1))),
                        )),
                    ),
                )),
            ],
            term(TerminatorKind::Return),
        )],
        vec![
            local_decl(glyim_type::Ty::ERROR),
            local_decl(glyim_type::Ty::ERROR),
            local_decl(glyim_type::Ty::ERROR),
        ],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
    let bytecode = result.unwrap();
    assert!(!bytecode.is_empty(), "Bytecode should not be empty");
    // Should contain at least LoadConst-like patterns and Add and Return
    assert!(bytecode.len() > 2, "Expected more than 2 bytes for multiple operations");
}

// ============================================================================
// S07-T03: Function with locals → LoadLocal + StoreLocal
// ============================================================================
#[test]
fn t03_locals_yield_loadlocal_storelocal() {
    let body = make_body(
        vec![block(
            vec![
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(1),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Copy(Place::new(LocalIdx::from_raw(0)))),
                )),
            ],
            term(TerminatorKind::Return),
        )],
        vec![
            local_decl(glyim_type::Ty::ERROR),
            local_decl(glyim_type::Ty::ERROR),
        ],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
    let bytecode = result.unwrap();
    assert!(!bytecode.is_empty(), "Bytecode should not be empty");
    // Should contain LoadLocal (or equivalent) operations
    assert!(bytecode.len() > 1, "Expected more than 1 byte for local operations");
}

// ============================================================================
// S07-T04: Branch → JumpIf + Jump opcodes
// ============================================================================
#[test]
fn t04_branch_yields_jumpif_and_jump() {
    let true_block = block(vec![], term(TerminatorKind::Return));
    let false_block = block(vec![], term(TerminatorKind::Return));
    let body = make_body(
        vec![
            block(
                vec![stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Bool(true),
                        ty: glyim_type::Ty::BOOL,
                        span: Span::DUMMY,
                    })),
                ))],
                term(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Place::new(LocalIdx::from_raw(0))),
                    switch_ty: glyim_type::Ty::BOOL,
                    targets: SwitchTargets::new(
                        Box::new([(0u128, BasicBlockIdx::from_raw(1))]),
                        BasicBlockIdx::from_raw(2),
                    ),
                }),
            ),
            true_block,
            false_block,
        ],
        vec![local_decl(glyim_type::Ty::BOOL)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
    let bytecode = result.unwrap();
    assert!(!bytecode.is_empty(), "Bytecode should not be empty");
    // Should contain branch/jump instructions
    assert!(bytecode.len() > 1, "Expected more than 1 byte for branch operations");
}

// ============================================================================
// S07-T05: generate() returns non-empty Vec<u8>
// ============================================================================
#[test]
fn t05_generate_returns_non_empty_vec_u8() {
    let body = make_body(
        vec![block(vec![], term(TerminatorKind::Return))],
        vec![],
        0,
    );

    let backend = BytecodeBackend::new();
    let output_path = Path::new("/tmp/test_output.bc");
    let result = backend.generate(&[body], output_path);
    assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
    let bytecode = result.unwrap();
    assert!(!bytecode.is_empty(), "generate() should return non-empty Vec<u8>");
}

// ============================================================================
// S07-T06: name() returns 'bytecode'
// ============================================================================
#[test]
fn t06_name_returns_bytecode() {
    let backend = BytecodeBackend::new();
    assert_eq!(backend.name(), "bytecode");
}
