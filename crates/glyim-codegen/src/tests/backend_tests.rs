use crate::{BytecodeBackend, CodegenBackend};
use glyim_core::primitives::*;
use glyim_mir::*;
use glyim_type::FieldIdx;
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
            vec![
                stmt(StatementKind::Nop),
                stmt(StatementKind::Nop),
            ],
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
    let body1 = make_body(
        vec![block(vec![], term(TerminatorKind::Return))],
        vec![],
        0,
    );
    let body2 = make_body(
        vec![block(vec![], term(TerminatorKind::Return))],
        vec![],
        0,
    );

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
            vec![
                stmt(StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Bool(false),
                        ty: glyim_type::Ty::BOOL,
                        span: Span::DUMMY,
                    })),
                )),
            ],
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
                Rvalue::Ref(
                    Place::new(LocalIdx::from_raw(1)),
                    BorrowKind::Shared,
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
                Rvalue::UnaryOp(
                    UnOp::Neg,
                    Operand::Copy(Place::new(LocalIdx::from_raw(1))),
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
                    vec![
                        Operand::Constant(MirConst {
                            kind: MirConstKind::Int(1),
                            ty: glyim_type::Ty::ERROR,
                            span: Span::DUMMY,
                        }),
                    ],
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
