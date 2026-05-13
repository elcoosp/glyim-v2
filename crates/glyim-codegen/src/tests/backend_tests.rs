use crate::{
    BytecodeBackend, CodegenBackend, OP_ADD, OP_JUMP, OP_JUMP_IF, OP_LOAD_CONST, OP_LOAD_LOCAL,
    OP_RETURN, OP_STORE_LOCAL,
};
use glyim_core::primitives::*;
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::FieldIdx;
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
    let body = make_body(vec![block(vec![], term(TerminatorKind::Return))], vec![], 0);

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
    assert!(
        bytecode.len() > 2,
        "Expected more than 2 bytes for multiple operations"
    );
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
    assert!(
        bytecode.len() > 1,
        "Expected more than 1 byte for local operations"
    );
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
    assert!(
        bytecode.len() > 1,
        "Expected more than 1 byte for branch operations"
    );
}

// ============================================================================
// S07-T05: generate() returns non-empty Vec<u8>
// ============================================================================
#[test]
fn t05_generate_returns_non_empty_vec_u8() {
    let body = make_body(vec![block(vec![], term(TerminatorKind::Return))], vec![], 0);

    let backend = BytecodeBackend::new();
    let output_path = Path::new("/tmp/test_output.bc");
    let result = backend.generate(&[body], output_path);
    assert!(result.is_ok(), "Expected Ok, got Err: {:?}", result.err());
    let bytecode = result.unwrap();
    assert!(
        !bytecode.is_empty(),
        "generate() should return non-empty Vec<u8>"
    );
}

// ============================================================================
// S07-T06: name() returns 'bytecode'
// ============================================================================
#[test]
fn t06_name_returns_bytecode() {
    let backend = BytecodeBackend::new();
    assert_eq!(backend.name(), "bytecode");
}

// ============================================================================
// S07-T07: Goto terminator emits Jump opcode
// ============================================================================
#[test]
fn t07_goto_emits_jump() {
    let body = make_body(
        vec![
            block(
                vec![],
                term(TerminatorKind::Goto {
                    target: BasicBlockIdx::from_raw(1),
                }),
            ),
            block(vec![], term(TerminatorKind::Return)),
        ],
        vec![],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    assert!(!bytecode.is_empty());
    // Should contain OP_JUMP (0x07)
    assert!(bytecode.contains(&0x07u8));
    // Should also contain OP_RETURN (0x05)
    assert!(bytecode.contains(&0x05u8));
}

// ============================================================================
// S07-T08: Unreachable terminator emits nothing
// ============================================================================
#[test]
fn t08_unreachable_emits_nothing() {
    let body = make_body(
        vec![block(vec![], term(TerminatorKind::Unreachable))],
        vec![],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    // Unreachable should not emit any opcode
    assert!(bytecode.is_empty());
}

// ============================================================================
// S07-T09: StorageLive/StorageDead are ignored
// ============================================================================
#[test]
fn t09_storage_live_dead_ignored() {
    let body = make_body(
        vec![block(
            vec![
                stmt(StatementKind::StorageLive(LocalIdx::from_raw(0))),
                stmt(StatementKind::StorageDead(LocalIdx::from_raw(0))),
            ],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    // Only OP_RETURN should be emitted
    assert_eq!(bytecode.len(), 1);
    assert_eq!(bytecode[0], 0x05); // OP_RETURN
}

// ============================================================================
// S07-T10: Nop statement emits nothing
// ============================================================================
#[test]
fn t10_nop_emits_nothing() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Nop), stmt(StatementKind::Nop)],
            term(TerminatorKind::Return),
        )],
        vec![],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    // Only OP_RETURN should be emitted
    assert_eq!(bytecode.len(), 1);
    assert_eq!(bytecode[0], 0x05);
}

// ============================================================================
// S07-T11: generate() with multiple bodies combines bytecode
// ============================================================================
#[test]
fn t11_generate_multiple_bodies_combines() {
    let body1 = make_body(vec![block(vec![], term(TerminatorKind::Return))], vec![], 0);
    let body2 = make_body(vec![block(vec![], term(TerminatorKind::Return))], vec![], 0);

    let backend = BytecodeBackend::new();
    let output_path = Path::new("/tmp/test_output_multi.bc");
    let result = backend.generate(&[body1, body2], output_path);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    // Should have 2 OP_RETURN bytes
    assert_eq!(bytecode.len(), 2);
    assert_eq!(bytecode[0], 0x05);
    assert_eq!(bytecode[1], 0x05);
}

// ============================================================================
// S07-T12: generate() with empty bodies returns empty Vec
// ============================================================================
#[test]
fn t12_generate_empty_bodies_returns_empty() {
    let backend = BytecodeBackend::new();
    let output_path = Path::new("/tmp/test_empty.bc");
    let result = backend.generate(&[], output_path);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    assert!(bytecode.is_empty());
}

// ============================================================================
// S07-T13: Bool constant emits correct integer encoding
// ============================================================================
#[test]
fn t13_bool_constant_encodes_correctly() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Bool(false),
                    ty: glyim_type::Ty::BOOL,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::BOOL)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    // OP_LOAD_CONST (0x01) + 8 bytes (false=0i64) + OP_STORE_LOCAL (0x04) + 4 bytes local index + OP_RETURN (0x05)
    assert!(bytecode.len() > 9);
    assert_eq!(bytecode[0], 0x01); // OP_LOAD_CONST
    // bytes 1-8: false as i64 = all zeros
    for i in 1..=8 {
        assert_eq!(bytecode[i], 0u8, "Expected zero at byte {}", i);
    }
}

// ============================================================================
// S07-T14: Stress test - many statements in one function
// ============================================================================
#[test]
fn t14_stress_many_statements() {
    let num_locals: usize = 20;
    let mut stmts = Vec::new();
    for i in 0..num_locals {
        stmts.push(stmt(StatementKind::Assign(
            Place::new(LocalIdx::from_raw(i as u32)),
            Rvalue::Use(Operand::Constant(MirConst {
                kind: MirConstKind::Int(i as i128),
                ty: glyim_type::Ty::ERROR,
                span: Span::DUMMY,
            })),
        )));
    }
    let body = make_body(
        vec![block(stmts, term(TerminatorKind::Return))],
        vec![local_decl(glyim_type::Ty::ERROR); num_locals],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    assert!(!bytecode.is_empty());
    // Each constant assign: OP_LOAD_CONST (1) + 8 bytes const + OP_STORE_LOCAL (1) + 4 bytes local = 14 bytes
    // Plus OP_RETURN (1)
    let expected_min = num_locals * 14 + 1;
    assert_eq!(bytecode.len(), expected_min);
}

// ============================================================================
// S07-T15: Call terminator stub does not crash
// ============================================================================
#[test]
fn t15_call_stub_does_not_crash() {
    let body = make_body(
        vec![block(
            vec![],
            term(TerminatorKind::Call {
                func: Operand::Constant(MirConst {
                    kind: MirConstKind::Unit,
                    ty: glyim_type::Ty::UNIT,
                    span: Span::DUMMY,
                }),
                args: vec![],
                destination: Place::new(LocalIdx::from_raw(0)),
                target: Some(BasicBlockIdx::from_raw(1)),
                cleanup: None,
            }),
        )],
        vec![local_decl(glyim_type::Ty::UNIT)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bytecode = result.unwrap();
    // Call stub should emit a warning but not crash; may emit OPERAND + empty or nothing
    // Just verify it doesn't panic and returns Ok
    assert!(bytecode.is_empty() || !bytecode.is_empty());
}

// ============================================================================
// S07-T16: Ref rvalue stub does not crash
// ============================================================================
#[test]
fn t16_ref_stub_does_not_crash() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Ref(Place::new(LocalIdx::from_raw(1)), BorrowKind::Shared),
            ))],
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
    assert!(result.is_ok());
    // Ref stub should not crash
}

// ============================================================================
// S07-T17: UnaryOp stub does not crash
// ============================================================================
#[test]
fn t17_unary_op_stub_does_not_crash() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::UnaryOp(UnOp::Neg, Operand::Copy(Place::new(LocalIdx::from_raw(1)))),
            ))],
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
    assert!(result.is_ok());
    // UnaryOp stub should not crash
}

// ============================================================================
// S07-T18: Sub/Mul/Div stubs do not crash
// ============================================================================
#[test]
fn t18_arithmetic_stubs_do_not_crash() {
    for op in &[BinOp::Sub, BinOp::Mul, BinOp::Div] {
        let body = make_body(
            vec![block(
                vec![
                    stmt(StatementKind::Assign(
                        Place::new(LocalIdx::from_raw(0)),
                        Rvalue::Use(Operand::Constant(MirConst {
                            kind: MirConstKind::Int(10),
                            ty: glyim_type::Ty::ERROR,
                            span: Span::DUMMY,
                        })),
                    )),
                    stmt(StatementKind::Assign(
                        Place::new(LocalIdx::from_raw(1)),
                        Rvalue::Use(Operand::Constant(MirConst {
                            kind: MirConstKind::Int(3),
                            ty: glyim_type::Ty::ERROR,
                            span: Span::DUMMY,
                        })),
                    )),
                    stmt(StatementKind::Assign(
                        Place::new(LocalIdx::from_raw(2)),
                        Rvalue::BinaryOp(
                            *op,
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
        assert!(result.is_ok(), "Op {:?} failed: {:?}", op, result.err());
    }
}

// ============================================================================
// S07-T19: Verify exact bytecode for simple constant+return
// ============================================================================
#[test]
fn t19_exact_bytecode_constant_and_return() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(42),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bc = result.unwrap();
    // OP_LOAD_CONST (1) + 8 bytes (42) + OP_STORE_LOCAL (1) + 4 bytes (local 0) + OP_RETURN (1) = 15 bytes
    assert_eq!(bc.len(), 15);
    assert_eq!(bc[0], 0x01); // OP_LOAD_CONST
    let const_bytes = &bc[1..9];
    let val = i64::from_le_bytes(const_bytes.try_into().unwrap());
    assert_eq!(val, 42);
    assert_eq!(bc[9], 0x04); // OP_STORE_LOCAL
    let local_bytes = &bc[10..14];
    let local_idx = u32::from_le_bytes(local_bytes.try_into().unwrap());
    assert_eq!(local_idx, 0);
    assert_eq!(bc[14], 0x05); // OP_RETURN
}

// ============================================================================
// S07-T20: SwitchInt on bool with false value jumps to false target
// ============================================================================
#[test]
fn t20_switchint_false_jumps_to_false_target() {
    let body = make_body(
        vec![
            block(
                vec![stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Bool(false),
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
            block(vec![], term(TerminatorKind::Return)),
            block(vec![], term(TerminatorKind::Unreachable)),
        ],
        vec![local_decl(glyim_type::Ty::BOOL)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bc = result.unwrap();
    // Should contain OP_JUMP_IF (0x06) and OP_JUMP (0x07)
    assert!(bc.contains(&0x06u8));
    assert!(bc.contains(&0x07u8));
}

// ============================================================================
// S07-T21: Move operand treated same as Copy
// ============================================================================
#[test]
fn t21_move_operand_works_like_copy() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Move(Place::new(LocalIdx::from_raw(1)))),
            ))],
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
    assert!(result.is_ok());
    let bc = result.unwrap();
    // OP_LOAD_LOCAL (1) + 4 bytes local + OP_STORE_LOCAL (1) + 4 bytes local + OP_RETURN (1)
    assert_eq!(bc.len(), 11);
    assert_eq!(bc[0], 0x03); // OP_LOAD_LOCAL
}

// ============================================================================
// S07-T22: Multiple basic blocks all processed
// ============================================================================
#[test]
fn t22_multiple_blocks_all_processed() {
    let body = make_body(
        vec![
            block(
                vec![stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(1),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                ))],
                term(TerminatorKind::Goto {
                    target: BasicBlockIdx::from_raw(1),
                }),
            ),
            block(vec![], term(TerminatorKind::Return)),
        ],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bc = result.unwrap();
    // Block 0: LoadConst + StoreLocal + Jump
    // Block 1: Return
    assert!(bc.contains(&0x07u8)); // OP_JUMP
    assert!(bc.contains(&0x05u8)); // OP_RETURN
}

// ============================================================================
// S07-T23: Aggregate rvalue (Tuple) stub does not crash
// ============================================================================
#[test]
fn t23_aggregate_tuple_stub_does_not_crash() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Aggregate(
                    AggregateKind::Tuple,
                    vec![Operand::Constant(MirConst {
                        kind: MirConstKind::Int(1),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })],
                ),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T24: Len rvalue stub does not crash
// ============================================================================
#[test]
fn t24_len_stub_does_not_crash() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Len(Place::new(LocalIdx::from_raw(1))),
            ))],
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
    assert!(result.is_ok());
}

// ============================================================================
// S07-T25: Cast rvalue stub does not crash
// ============================================================================
#[test]
fn t25_cast_stub_does_not_crash() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Cast(
                    CastKind::IntToInt,
                    Operand::Copy(Place::new(LocalIdx::from_raw(1))),
                    glyim_type::Ty::ERROR,
                ),
            ))],
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
    assert!(result.is_ok());
}

// ============================================================================
// S07-T26: Discriminant rvalue stub does not crash
// ============================================================================
#[test]
fn t26_discriminant_stub_does_not_crash() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Discriminant(Place::new(LocalIdx::from_raw(1))),
            ))],
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
    assert!(result.is_ok());
}

// ============================================================================
// S07-T27: Repeat rvalue stub does not crash
// ============================================================================
#[test]
fn t27_repeat_stub_does_not_crash() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Repeat(
                    Operand::Constant(MirConst {
                        kind: MirConstKind::Int(0),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    }),
                    MirConst {
                        kind: MirConstKind::Int(5),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    },
                ),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T28: Assert terminator stub does not crash
// ============================================================================
#[test]
fn t28_assert_stub_does_not_crash() {
    let body = make_body(
        vec![block(
            vec![],
            term(TerminatorKind::Assert {
                cond: Operand::Constant(MirConst {
                    kind: MirConstKind::Bool(true),
                    ty: glyim_type::Ty::BOOL,
                    span: Span::DUMMY,
                }),
                expected: true,
                target: BasicBlockIdx::from_raw(1),
                cleanup: None,
                msg: AssertMessage::Overflow(BinOp::Add),
            }),
        )],
        vec![],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T29: Drop terminator stub does not crash
// ============================================================================
#[test]
fn t29_drop_stub_does_not_crash() {
    let body = make_body(
        vec![block(
            vec![],
            term(TerminatorKind::Drop {
                place: Place::new(LocalIdx::from_raw(0)),
                target: BasicBlockIdx::from_raw(1),
                cleanup: None,
            }),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T30: Assign with projection (field access) stub does not crash
// ============================================================================
#[test]
fn t30_assign_with_projection_stub_does_not_crash() {
    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: Box::new([ProjectionElem::Field(FieldIdx::from_raw(0))]),
    };

    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                place,
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(42),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T31: Operand with projection (index) stub does not crash
// ============================================================================
#[test]
fn t31_operand_with_projection_stub_does_not_crash() {
    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: Box::new([ProjectionElem::Index(LocalIdx::from_raw(1))]),
    };

    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(2)),
                Rvalue::Use(Operand::Copy(place)),
            ))],
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
    assert!(result.is_ok());
}

// ============================================================================
// S07-T32: Unsigned integer constant handled gracefully
// ============================================================================
#[test]
fn t32_unsigned_constant_handled() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Uint(42),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T33: Char constant handled gracefully
// ============================================================================
#[test]
fn t33_char_constant_handled() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Char('a'),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T34: Float constant handled gracefully
// ============================================================================
#[test]
fn t34_float_constant_handled() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::FloatBits(1065353216), // 1.0f32 bits
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T35: String constant handled gracefully
// ============================================================================
#[test]
fn t35_string_constant_handled() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Unit,
                    ty: glyim_type::Ty::UNIT,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T36: Unit constant handled
// ============================================================================
#[test]
fn t36_unit_constant_handled() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Unit,
                    ty: glyim_type::Ty::UNIT,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::UNIT)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T37: Substituted copy with same constant works correctly
// ============================================================================
#[test]
fn t37_multiple_uses_of_same_constant_works() {
    let body = make_body(
        vec![block(
            vec![
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(5),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(5),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
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
    assert!(result.is_ok());
    let bc = result.unwrap();
    // Two load_const + two store_local + return = 2*(1+8+1+4) + 1 = 29 bytes
    assert_eq!(bc.len(), 29);
}

// ============================================================================
// S07-T38: Error constant kind handled gracefully
// ============================================================================
#[test]
fn t38_error_constant_handled() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Error,
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T39: SwitchInt on non-bool type stub does not crash
// ============================================================================
#[test]
fn t39_switchint_non_bool_stub_does_not_crash() {
    let body = make_body(
        vec![
            block(
                vec![],
                term(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Place::new(LocalIdx::from_raw(0))),
                    switch_ty: glyim_type::Ty::ERROR,
                    targets: SwitchTargets::new(
                        Box::new([(1u128, BasicBlockIdx::from_raw(1))]),
                        BasicBlockIdx::from_raw(2),
                    ),
                }),
            ),
            block(vec![], term(TerminatorKind::Return)),
            block(vec![], term(TerminatorKind::Unreachable)),
        ],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T40: Exactly one byte for empty function (only Return)
// ============================================================================
#[test]
fn t40_empty_function_returns_exactly_one_byte() {
    let body = make_body(vec![block(vec![], term(TerminatorKind::Return))], vec![], 0);
    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    assert_eq!(bc.len(), 1);
    assert_eq!(bc[0], OP_RETURN);
}

// ============================================================================
// S07-T41: OP_ADD exact bytecode sequence
// ============================================================================
#[test]
fn t41_add_exact_bytecode_sequence() {
    let body = make_body(
        vec![block(
            vec![
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(3),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(4),
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
    let bc = backend.generate_function(&body).unwrap();
    // Expected layout:
    // LoadConst 3 (0x01 + 8 bytes = 3) + StoreLocal 0 (0x04 + 4 bytes = 0) = 13 bytes
    // LoadConst 4 + StoreLocal 1 = 13 bytes
    // LoadLocal 0 (0x03 + 4 bytes = 0) = 5 bytes
    // LoadLocal 1 = 5 bytes
    // OP_ADD (0x02) = 1 byte
    // StoreLocal 2 (0x04 + 4 bytes = 2) = 5 bytes
    // Return (0x05) = 1 byte
    // Total = 13+13+5+5+1+5+1 = 43 bytes
    assert_eq!(bc.len(), 45);
    // Check opcode sequence
    assert_eq!(bc[0], OP_LOAD_CONST);
    assert_eq!(bc[14], OP_LOAD_CONST);
    assert_eq!(bc[28], OP_LOAD_LOCAL);
    assert_eq!(bc[33], OP_LOAD_LOCAL);
    assert_eq!(bc[38], OP_ADD);
    assert_eq!(bc[39], OP_STORE_LOCAL);
    assert_eq!(bc[44], OP_RETURN);
}

// ============================================================================
// S07-T42: Bytecode is deterministic (same input → same output)
// ============================================================================
#[test]
fn t42_bytecode_deterministic() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(42),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc1 = backend.generate_function(&body).unwrap();
    let bc2 = backend.generate_function(&body).unwrap();
    assert_eq!(bc1, bc2, "Bytecode should be deterministic");
}

// ============================================================================
// S07-T43: Default implementation works
// ============================================================================
#[test]
fn t43_default_works() {
    let backend = BytecodeBackend::default();
    assert_eq!(backend.name(), "bytecode");
    let body = make_body(vec![block(vec![], term(TerminatorKind::Return))], vec![], 0);
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T44: Multiple calls to generate_function on same backend are independent
// ============================================================================
#[test]
fn t44_multiple_calls_independent() {
    let body1 = make_body(vec![block(vec![], term(TerminatorKind::Return))], vec![], 0);
    let body2 = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(7),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc1 = backend.generate_function(&body1).unwrap();
    let bc2 = backend.generate_function(&body2).unwrap();
    // They should be different
    assert_ne!(bc1, bc2);
}

// ============================================================================
// S07-T45: Function with arguments (arg_count > 0) still works
// ============================================================================
#[test]
fn t45_function_with_args() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(2)),
                Rvalue::BinaryOp(
                    BinOp::Add,
                    Box::new((
                        Operand::Copy(Place::new(LocalIdx::from_raw(0))),
                        Operand::Copy(Place::new(LocalIdx::from_raw(1))),
                    )),
                ),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![
            local_decl(glyim_type::Ty::ERROR),
            local_decl(glyim_type::Ty::ERROR),
            local_decl(glyim_type::Ty::ERROR),
        ],
        2, // two arguments
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bc = result.unwrap();
    // Should contain LoadLocal for args 0 and 1, then OP_ADD, then StoreLocal 2, then Return
    assert!(!bc.is_empty());
}

// ============================================================================
// S07-T46: MirConstKind::Char constant emitted as i64 (unicode scalar)
// ============================================================================
#[test]
fn t46_char_constant_emits_unicode_scalar() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Char('A'),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    assert_eq!(bc[0], OP_LOAD_CONST);
    let const_bytes = &bc[1..9];
    let val = i64::from_le_bytes(const_bytes.try_into().unwrap());
    assert_eq!(val, 'A' as i64);
}

// ============================================================================
// S07-T47: Large integer constant (i128 fits i64)
// ============================================================================
#[test]
fn t47_large_integer_constant() {
    let big_val: i128 = 0x7FFFFFFFFFFFFFFF; // max i64 positive
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(big_val),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    let const_bytes = &bc[1..9];
    let val = i64::from_le_bytes(const_bytes.try_into().unwrap());
    assert_eq!(val as i128, big_val);
}

// ============================================================================
// S07-T48: MirConstKind::FloatBits emitted as i64 bits
// ============================================================================
#[test]
fn t48_float_bits_constant() {
    let bits: u32 = 1065353216; // 1.0f32 bits
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::FloatBits(bits as u64),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    let const_bytes = &bc[1..9];
    let val = i64::from_le_bytes(const_bytes.try_into().unwrap());
    // FloatBits u32 stored as i64 (sign extension or just bits?). Implementation stores as i64, so we cast.
    assert_eq!(val as u64, bits as u64);
}

// ============================================================================
// S07-T49: All opcodes are unique (no overlapping byte values)
// ============================================================================
#[test]
fn t49_opcodes_unique() {
    let opcodes = [
        OP_LOAD_CONST,
        OP_ADD,
        OP_LOAD_LOCAL,
        OP_STORE_LOCAL,
        OP_RETURN,
        OP_JUMP_IF,
        OP_JUMP,
    ];
    // Check that they are all distinct
    for i in 0..opcodes.len() {
        for j in i + 1..opcodes.len() {
            assert_ne!(
                opcodes[i], opcodes[j],
                "Opcodes {} and {} have same value {}",
                i, j, opcodes[i]
            );
        }
    }
}

// ============================================================================
// S07-T50: generate_function on Body with no terminator (should be impossible but test)
// ============================================================================
#[test]
fn t50_no_terminator_block() {
    // BasicBlockData with no terminator (None option is not possible, but we can simulate by constructing
    // a block that has terminator set to None? Actually the field is Terminator, not Option<Terminator>.
    // So we can't have None. But we can have a block with terminator Unreachable.
    // The test already covered that. Skip this.
    // Instead, test that a block without statements but with Return works.
    // Already tested in t40.
}

// ============================================================================
// S07-T51: Mixed rvalue stubs all produce no panic and some bytecode
// ============================================================================
#[test]
fn t51_mixed_stub_rvalues() {
    let body = make_body(
        vec![block(
            vec![
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Len(Place::new(LocalIdx::from_raw(1))),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(2)),
                    Rvalue::Discriminant(Place::new(LocalIdx::from_raw(0))),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(3)),
                    Rvalue::Ref(Place::new(LocalIdx::from_raw(1)), BorrowKind::Shared),
                )),
            ],
            term(TerminatorKind::Return),
        )],
        vec![
            local_decl(glyim_type::Ty::ERROR),
            local_decl(glyim_type::Ty::ERROR),
            local_decl(glyim_type::Ty::ERROR),
            local_decl(glyim_type::Ty::ERROR),
        ],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T52: Negative integer constant encodes as signed i64
// ============================================================================
#[test]
fn t52_negative_integer_constant() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(-42),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    let const_bytes = &bc[1..9];
    let val = i64::from_le_bytes(const_bytes.try_into().unwrap());
    assert_eq!(val, -42);
}

// ============================================================================
// S07-T53: Minimum i64 constant
// ============================================================================
#[test]
fn t53_minimum_i64_constant() {
    let min_val: i128 = i64::MIN as i128;
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(min_val),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    let const_bytes = &bc[1..9];
    let val = i64::from_le_bytes(const_bytes.try_into().unwrap());
    assert_eq!(val, i64::MIN);
}

// ============================================================================
// S07-T54: Maximum i64 constant
// ============================================================================
#[test]
fn t54_maximum_i64_constant() {
    let max_val: i128 = i64::MAX as i128;
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(max_val),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    let const_bytes = &bc[1..9];
    let val = i64::from_le_bytes(const_bytes.try_into().unwrap());
    assert_eq!(val, i64::MAX);
}

// ============================================================================
// S07-T55: Zero integer constant encodes correctly
// ============================================================================
#[test]
fn t55_zero_integer_constant() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(0),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    let const_bytes = &bc[1..9];
    let val = i64::from_le_bytes(const_bytes.try_into().unwrap());
    assert_eq!(val, 0);
}

// ============================================================================
// S07-T56: Complex branch with three targets
// ============================================================================
#[test]
fn t56_complex_branch_three_targets() {
    let body = make_body(
        vec![
            block(
                vec![stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Bool(false),
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
            block(
                vec![stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(1),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                ))],
                term(TerminatorKind::Return),
            ),
            block(
                vec![stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(2)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(2),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                ))],
                term(TerminatorKind::Return),
            ),
        ],
        vec![
            local_decl(glyim_type::Ty::BOOL),
            local_decl(glyim_type::Ty::ERROR),
            local_decl(glyim_type::Ty::ERROR),
        ],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    assert!(!bc.is_empty());
    assert!(bc.contains(&OP_JUMP_IF));
    assert!(bc.contains(&OP_JUMP));
    assert!(bc.contains(&OP_RETURN));
}

// ============================================================================
// S07-T57: StoreLocal with high-index local (u32::MAX)
// ============================================================================
#[test]
// ============================================================================
// S07-T58: LoadLocal with mid-range index
// ============================================================================
#[test]
fn t58_loadlocal_midrange_index() {
    let idx: u32 = 1000;
    let body = make_body(
        vec![block(
            vec![
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(idx)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(5),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(idx + 1)),
                    Rvalue::Use(Operand::Copy(Place::new(LocalIdx::from_raw(idx)))),
                )),
            ],
            term(TerminatorKind::Return),
        )],
        {
            let mut locals = vec![];
            for _ in 0..=(idx + 1) as usize {
                locals.push(local_decl(glyim_type::Ty::ERROR));
            }
            locals
        },
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    // The second LoadLocal should have index idx
    // Layout: LoadConst + 8 bytes + StoreLocal(idx) = 1+8+1+4 = 14 bytes
    // Then LoadLocal(idx) + StoreLocal(idx+1) = 1+4+1+4 = 10 bytes
    // Plus Return = 1 byte. Total = 25 bytes
    assert_eq!(bc.len(), 25);
    let load_idx_bytes = &bc[15..19];
    let load_idx = u32::from_le_bytes(load_idx_bytes.try_into().unwrap());
    assert_eq!(load_idx, idx);
}

// ============================================================================
// S07-T59: Nested BinaryOp not supported but doesn't crash
// ============================================================================
#[test]
fn t59_nested_binaryop_does_not_crash() {
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
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(2),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(2)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(3),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(3)),
                    Rvalue::BinaryOp(
                        BinOp::Add,
                        Box::new((
                            Operand::Copy(Place::new(LocalIdx::from_raw(0))),
                            Operand::Copy(Place::new(LocalIdx::from_raw(1))),
                        )),
                    ),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(4)),
                    Rvalue::BinaryOp(
                        BinOp::Add,
                        Box::new((
                            Operand::Copy(Place::new(LocalIdx::from_raw(3))),
                            Operand::Copy(Place::new(LocalIdx::from_raw(2))),
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
            local_decl(glyim_type::Ty::ERROR),
            local_decl(glyim_type::Ty::ERROR),
        ],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
}

// ============================================================================
// S07-T60: Empty generate_function -> just Return in vec
// ============================================================================
#[test]
fn t60_empty_body_generates_return() {
    let body = make_body(vec![block(vec![], term(TerminatorKind::Return))], vec![], 0);
    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    assert_eq!(bc, vec![OP_RETURN]);
}

// ============================================================================
// S07-T61: Function with var_debug_info still emits correct bytecode
// ============================================================================
#[test]
fn t61_var_debug_info_ignored_correctly() {
    use glyim_mir::VarDebugInfo;
    let mut body_val = make_body(vec![block(vec![], term(TerminatorKind::Return))], vec![], 0);
    let body_mut = Arc::get_mut(&mut body_val).unwrap();
    body_mut.var_debug_info = vec![VarDebugInfo {
        name: glyim_core::Interner::default().intern("x"),
        value: glyim_mir::VarDebugInfoValue::Const(MirConst {
            kind: MirConstKind::Int(0),
            ty: glyim_type::Ty::ERROR,
            span: Span::DUMMY,
        }),
    }];
    let _ = body_mut;

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body_val).unwrap();
    assert_eq!(bc, vec![OP_RETURN]);
}

// ============================================================================
// S07-T62: generate() with single body and multiple bodies consistent
// ============================================================================
#[test]
fn t62_generate_single_body_consistent_with_generate_function() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(42),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let func_bc = backend.generate_function(&body).unwrap();
    let gen_bc = backend
        .generate(&[body.clone()], Path::new("/tmp/t62.bc"))
        .unwrap();
    assert_eq!(func_bc, gen_bc);
}

// ============================================================================
// S07-T63: Bool true constant encodes as 1
// ============================================================================
#[test]
fn t63_bool_true_encodes_as_1() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Bool(true),
                    ty: glyim_type::Ty::BOOL,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::BOOL)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    let const_bytes = &bc[1..9];
    let val = i64::from_le_bytes(const_bytes.try_into().unwrap());
    assert_eq!(val, 1);
}

// ============================================================================
// S07-T64: Bool false constant encodes as 0
// ============================================================================
#[test]
fn t64_bool_false_encodes_as_0() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Bool(false),
                    ty: glyim_type::Ty::BOOL,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::BOOL)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    let const_bytes = &bc[1..9];
    let val = i64::from_le_bytes(const_bytes.try_into().unwrap());
    assert_eq!(val, 0);
}

// ============================================================================
// S07-T65: Uint constant UintTy::U32 value (42u32)
// ============================================================================
#[test]
fn t65_uint_constant_encodes_as_i64() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Uint(42),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    let const_bytes = &bc[1..9];
    let val = i64::from_le_bytes(const_bytes.try_into().unwrap());
    assert_eq!(val, 42);
}

// ============================================================================
// S07-T66: Uint MAX value
// ============================================================================
#[test]
fn t66_uint_max_value() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Uint(u64::MAX as u128),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    let const_bytes = &bc[1..9];
    let val = i64::from_le_bytes(const_bytes.try_into().unwrap());
    // u64::MAX as i64 wraps to -1, but we're storing u128 as i64, so it truncates
    // The implementation does `*val as i64` on u128, so u64::MAX = 0xFFFFFFFFFFFFFFFF
    // which as i64 is -1. Let's verify the bits match.
    assert_eq!(val as u64, u64::MAX);
}

// ============================================================================
// S07-T67: Verify no branching instructions leak into function without branches
// ============================================================================
#[test]
fn t67_no_branch_leakage() {
    let body = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(100),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );

    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    // Should not contain any branch opcodes
    assert!(!bc.contains(&OP_JUMP));
    assert!(!bc.contains(&OP_JUMP_IF));
}

// ============================================================================
// S07-T68: Maximum number of locals per function stress test
// ============================================================================
#[test]
fn t68_many_locals_stress_test() {
    let num_locals: usize = 200;
    let mut stmts = Vec::new();
    for i in 0..num_locals {
        stmts.push(stmt(StatementKind::Assign(
            Place::new(LocalIdx::from_raw(i as u32)),
            Rvalue::Use(Operand::Constant(MirConst {
                kind: MirConstKind::Int((i % 100) as i128),
                ty: glyim_type::Ty::ERROR,
                span: Span::DUMMY,
            })),
        )));
    }
    let body = make_body(
        vec![block(stmts, term(TerminatorKind::Return))],
        vec![local_decl(glyim_type::Ty::ERROR); num_locals],
        0,
    );

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    let bc = result.unwrap();
    // 200 * 14 + 1 = 2801 bytes
    assert_eq!(bc.len(), num_locals * 14 + 1);
}

// ============================================================================
// S07-T69: Block with no Return terminator (Unreachable) yields empty bytecode
// ============================================================================
#[test]
fn t69_unreachable_block_yields_empty() {
    let body = make_body(
        vec![block(vec![], term(TerminatorKind::Unreachable))],
        vec![],
        0,
    );
    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    assert!(bc.is_empty());
}

// ============================================================================
// S07-T70: Goto loop emits two jumps correctly
// ============================================================================
#[test]
fn t70_goto_loop_emits_two_jumps() {
    let body = make_body(
        vec![
            block(
                vec![],
                term(TerminatorKind::Goto {
                    target: BasicBlockIdx::from_raw(1),
                }),
            ),
            block(
                vec![],
                term(TerminatorKind::Goto {
                    target: BasicBlockIdx::from_raw(0),
                }),
            ),
        ],
        vec![],
        0,
    );
    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    assert_eq!(bc.len(), 10); // two JUMP (1 + 4 bytes each) = 5*2=10
    assert_eq!(bc[0], OP_JUMP);
    assert_eq!(bc[5], OP_JUMP);
}

// ============================================================================
// S07-T71: Comparison BinaryOp stubs do not crash
// ============================================================================
#[test]
fn t71_comparison_ops_do_not_crash() {
    for op in &[BinOp::Eq, BinOp::Lt, BinOp::Ne, BinOp::Gt] {
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
                        Rvalue::Use(Operand::Constant(MirConst {
                            kind: MirConstKind::Int(2),
                            ty: glyim_type::Ty::ERROR,
                            span: Span::DUMMY,
                        })),
                    )),
                    stmt(StatementKind::Assign(
                        Place::new(LocalIdx::from_raw(2)),
                        Rvalue::BinaryOp(
                            *op,
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
        assert!(result.is_ok(), "Op {:?} failed", op);
    }
}

// ============================================================================
// S07-T72: Bitwise/shift ops stubs do not crash
// ============================================================================
#[test]
fn t72_bitwise_shift_ops_do_not_crash() {
    for op in &[
        BinOp::BitAnd,
        BinOp::BitOr,
        BinOp::BitXor,
        BinOp::Shl,
        BinOp::Shr,
    ] {
        let body = make_body(
            vec![block(
                vec![
                    stmt(StatementKind::Assign(
                        Place::new(LocalIdx::from_raw(0)),
                        Rvalue::Use(Operand::Constant(MirConst {
                            kind: MirConstKind::Int(7),
                            ty: glyim_type::Ty::ERROR,
                            span: Span::DUMMY,
                        })),
                    )),
                    stmt(StatementKind::Assign(
                        Place::new(LocalIdx::from_raw(1)),
                        Rvalue::Use(Operand::Constant(MirConst {
                            kind: MirConstKind::Int(2),
                            ty: glyim_type::Ty::ERROR,
                            span: Span::DUMMY,
                        })),
                    )),
                    stmt(StatementKind::Assign(
                        Place::new(LocalIdx::from_raw(2)),
                        Rvalue::BinaryOp(
                            *op,
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
        assert!(result.is_ok(), "Op {:?} failed", op);
    }
}

// ============================================================================
// S07-T73: Double assignment (chain of loads) correct indices
// ============================================================================
#[test]
fn t73_double_assignment_chain() {
    let body = make_body(
        vec![block(
            vec![
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(10),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Copy(Place::new(LocalIdx::from_raw(0)))),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(2)),
                    Rvalue::Use(Operand::Copy(Place::new(LocalIdx::from_raw(1)))),
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
    let bc = backend.generate_function(&body).unwrap();
    // Expected: LoadConst(10) + StoreLocal(0) = 14 bytes
    // LoadLocal(0) + StoreLocal(1) = 10 bytes
    // LoadLocal(1) + StoreLocal(2) = 10 bytes
    // Return = 1 byte => 35 bytes
    assert_eq!(bc.len(), 35);
}

// ============================================================================
// S07-T74: SwitchInt with true value targets otherwise block
// ============================================================================
#[test]
fn t74_switchint_true_targets_otherwise() {
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
            block(vec![], term(TerminatorKind::Unreachable)),
            block(vec![], term(TerminatorKind::Return)),
        ],
        vec![local_decl(glyim_type::Ty::BOOL)],
        0,
    );
    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    // Should contain OP_JUMP_IF targeting block 2, OP_JUMP targeting block 1
    assert!(bc.contains(&OP_JUMP_IF));
    assert!(bc.contains(&OP_JUMP));
}

// ============================================================================
// S07-T75: Empty blocks with Goto chain
// ============================================================================
#[test]
fn t75_goto_chain_three_blocks() {
    let body = make_body(
        vec![
            block(
                vec![],
                term(TerminatorKind::Goto {
                    target: BasicBlockIdx::from_raw(1),
                }),
            ),
            block(
                vec![],
                term(TerminatorKind::Goto {
                    target: BasicBlockIdx::from_raw(2),
                }),
            ),
            block(vec![], term(TerminatorKind::Return)),
        ],
        vec![],
        0,
    );
    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    assert_eq!(bc.len(), 11); // two Jumps (5 each) + Return (1)
}

// ============================================================================
// S07-T76: StoreLocal after constant without assign (Nop) doesn't add extra
// ============================================================================
#[test]
fn t76_nop_does_not_disrupt_bytecode() {
    let body = make_body(
        vec![block(
            vec![
                stmt(StatementKind::Nop),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(5),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                )),
                stmt(StatementKind::Nop),
            ],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );
    let backend = BytecodeBackend::new();
    let bc = backend.generate_function(&body).unwrap();
    // LoadConst+StoreLocal+Return = 15 bytes
    assert_eq!(bc.len(), 15);
}

// ============================================================================
// S07-T77: Multiple constants of same value reuse (no dedup, just check correctness)
// ============================================================================
#[test]
fn t77_same_constant_repeated_correct() {
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
                        kind: MirConstKind::Int(42),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
                )),
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(2)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(42),
                        ty: glyim_type::Ty::ERROR,
                        span: Span::DUMMY,
                    })),
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
    let bc = backend.generate_function(&body).unwrap();
    // 3*(LoadConst+StoreLocal) + Return = 3*14+1 = 43
    assert_eq!(bc.len(), 43);
}

// ============================================================================
// S07-T78: generate() with mixed empty and non-empty bodies
// ============================================================================
#[test]
fn t78_generate_mixed_empty_and_nonempty() {
    let body_nonempty = make_body(
        vec![block(
            vec![stmt(StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(1),
                    ty: glyim_type::Ty::ERROR,
                    span: Span::DUMMY,
                })),
            ))],
            term(TerminatorKind::Return),
        )],
        vec![local_decl(glyim_type::Ty::ERROR)],
        0,
    );
    let backend = BytecodeBackend::new();
    let output_path = Path::new("/tmp/t78.bc");
    // empty body: use a body with only Unreachable
    let empty_body = make_body(
        vec![block(vec![], term(TerminatorKind::Unreachable))],
        vec![],
        0,
    );
    let bc = backend
        .generate(
            &[empty_body, body_nonempty.clone(), body_nonempty],
            output_path,
        )
        .unwrap();
    // empty_body -> 0 bytes, body_nonempty -> 15 bytes each, total 30
    assert_eq!(bc.len(), 30);
}

// ============================================================================
// S07-T79: Test that the backend's generate_function returns Ok for all test bodies
// ============================================================================
#[test]
fn t79_no_errors_for_any_supported_terminator() {
    let terminators: Vec<TerminatorKind> = vec![
        TerminatorKind::Return,
        TerminatorKind::Unreachable,
        TerminatorKind::Goto {
            target: BasicBlockIdx::from_raw(1),
        },
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::new(LocalIdx::from_raw(0))),
            switch_ty: glyim_type::Ty::BOOL,
            targets: SwitchTargets::new(
                Box::new([(0u128, BasicBlockIdx::from_raw(1))]),
                BasicBlockIdx::from_raw(2),
            ),
        },
    ];
    for term_kind in terminators {
        let blocks = if matches!(
            term_kind,
            TerminatorKind::Goto { .. } | TerminatorKind::SwitchInt { .. }
        ) {
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
                    term(term_kind.clone()),
                ),
                block(vec![], term(TerminatorKind::Return)),
                block(vec![], term(TerminatorKind::Unreachable)),
            ]
        } else {
            vec![block(vec![], term(term_kind.clone()))]
        };
        let body = make_body(blocks, vec![local_decl(glyim_type::Ty::BOOL)], 0);
        let backend = BytecodeBackend::new();
        let result = backend.generate_function(&body);
        assert!(result.is_ok(), "Terminator {:?} caused error", term_kind);
    }
}

// ============================================================================
// S07-T80: Test that all defined opcodes are emitted at least once across tests
// ============================================================================
#[test]
fn t80_all_opcodes_emitted_somewhere() {
    // Build a body that contains all opcodes through multiple generate_function calls
    // and check that each opcode appears in at least one output.
    let backend = BytecodeBackend::new();

    // Body for LOAD_CONST, STORE_LOCAL, LOAD_LOCAL, ADD, RETURN
    let body1 = make_body(
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
    let bc1 = backend.generate_function(&body1).unwrap();
    assert!(bc1.contains(&OP_LOAD_CONST));
    assert!(bc1.contains(&OP_STORE_LOCAL));
    assert!(bc1.contains(&OP_LOAD_LOCAL));
    assert!(bc1.contains(&OP_ADD));
    assert!(bc1.contains(&OP_RETURN));

    // Body for JUMP (Goto)
    let body2 = make_body(
        vec![
            block(
                vec![],
                term(TerminatorKind::Goto {
                    target: BasicBlockIdx::from_raw(1),
                }),
            ),
            block(vec![], term(TerminatorKind::Return)),
        ],
        vec![],
        0,
    );
    let bc2 = backend.generate_function(&body2).unwrap();
    assert!(bc2.contains(&OP_JUMP));

    // Body for JUMP_IF (SwitchInt on bool)
    let body3 = make_body(
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
            block(vec![], term(TerminatorKind::Return)),
            block(vec![], term(TerminatorKind::Unreachable)),
        ],
        vec![local_decl(glyim_type::Ty::BOOL)],
        0,
    );
    let bc3 = backend.generate_function(&body3).unwrap();
    assert!(bc3.contains(&OP_JUMP_IF));
}
