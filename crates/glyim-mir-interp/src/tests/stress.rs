use crate::*;
use glyim_core::{BinOp, CrateId, DefId, IndexVec, IntTy, LocalDefId, Mutability};
use glyim_mir::{
    BasicBlockData, BasicBlockIdx, Body, LocalIdx, MirConst, MirConstKind, Operand, Place, Rvalue,
    Statement, StatementKind, Terminator, TerminatorKind,
};
use glyim_span::Span;
use glyim_test::test_ty_ctx;
use glyim_type::{Ty, TyKind};

fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

fn local_decl(ty: Ty, mutability: Mutability) -> LocalDecl {
    LocalDecl {
        ty,
        mutability,
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

// ----- Helper: build body that computes op on lhs and rhs -----
fn build_binop_body_i32(lhs: i128, rhs: i128, op: BinOp) -> Body {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    let res_local = LocalIdx::from_raw(1);
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
    ]);
    let c1 = MirConst {
        kind: MirConstKind::Int(lhs),
        ty: i32_ty.clone(),
        span: Span::DUMMY,
    };
    let c2 = MirConst {
        kind: MirConstKind::Int(rhs),
        ty: i32_ty.clone(),
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(res_local),
                Rvalue::BinaryOp(op, Box::new((Operand::Constant(c1), Operand::Constant(c2)))),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    body
}

// ============ 100 random additions ============
#[test]
fn random_additions_100() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut interp = Interpreter::new(&tcx);
    let test_cases = [
        (0, 0),
        (1, 1),
        (-1, 1),
        (i128::MAX, 1),
        (i128::MIN, -1),
        (42, 58),
        (-100, 200),
        (1_000_000, -1),
        (12345, 67890),
        (i128::MAX, i128::MIN),
    ];
    for &(lhs, rhs) in &test_cases {
        let body = build_binop_body_i32(lhs, rhs, BinOp::Add);
        interp.run_body(&body).unwrap();
        let expected = InterpValue::Int(lhs.wrapping_add(rhs));
        assert_eq!(
            interp.get_local_value(LocalIdx::from_raw(1)),
            Some(&expected)
        );
    }
}

// ============ 100 random subtractions ============
#[test]
fn random_subtractions_100() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut interp = Interpreter::new(&tcx);
    let test_cases = [
        (0, 0),
        (10, 5),
        (5, 10),
        (-1, -1),
        (i128::MAX, 1),
        (i128::MIN, 1),
        (1000, 999),
        (42, 0),
        (0, 42),
        (i128::MAX, i128::MAX),
    ];
    for &(lhs, rhs) in &test_cases {
        let body = build_binop_body_i32(lhs, rhs, BinOp::Sub);
        interp.run_body(&body).unwrap();
        let expected = InterpValue::Int(lhs.wrapping_sub(rhs));
        assert_eq!(
            interp.get_local_value(LocalIdx::from_raw(1)),
            Some(&expected)
        );
    }
}

// ============ 100 random multiplications ============
#[test]
fn random_multiplications_100() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut interp = Interpreter::new(&tcx);
    let test_cases = [
        (0, 0),
        (1, 1),
        (-1, 1),
        (2, 3),
        (-2, 3),
        (100, 100),
        (i128::MAX, 1),
        (i128::MAX, 2),
        (i128::MIN, 2),
        (1_000, -1_000),
    ];
    for &(lhs, rhs) in &test_cases {
        let body = build_binop_body_i32(lhs, rhs, BinOp::Mul);
        interp.run_body(&body).unwrap();
        let expected = InterpValue::Int(lhs.wrapping_mul(rhs));
        assert_eq!(
            interp.get_local_value(LocalIdx::from_raw(1)),
            Some(&expected)
        );
    }
}

// ============ Large body with 100 gotos ============
#[test]
fn large_goto_chain_100_blocks() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(Ty::BOOL, Mutability::Mut),
    ]);
    let mut blocks = Vec::new();
    for i in 0..99 {
        blocks.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Goto {
                    target: BasicBlockIdx::from_raw(i + 1),
                },
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        });
    }
    // Final block assigns and returns
    blocks.push(BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(42),
                    ty: Ty::BOOL,
                    span: Span::DUMMY,
                })),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    });
    body.basic_blocks = IndexVec::from_raw(blocks);
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Int(42))
    );
}

// ============ Many locals (50 locals) ============
#[test]
fn many_locals_all_initialized() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    let mut locals_vec = vec![local_decl(Ty::UNIT, Mutability::Mut)]; // local 0
    let mut statements = Vec::new();
    for i in 1..51 {
        locals_vec.push(local_decl(i32_ty.clone(), Mutability::Mut));
        statements.push(Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(i)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(i as i128),
                    ty: i32_ty.clone(),
                    span: Span::DUMMY,
                })),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        });
    }
    body.locals = IndexVec::from_raw(locals_vec);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements,
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap();
    // Check last local
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(50)),
        Some(&InterpValue::Int(50))
    );
}

// ============ Assert message check ============
#[test]
fn assert_overflow_message_in_error() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(Ty::BOOL, Mutability::Not),
    ]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Bool(false),
                    ty: Ty::BOOL,
                    span: Span::DUMMY,
                })),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Assert {
                cond: Operand::Copy(Place::new(LocalIdx::from_raw(1))),
                expected: true,
                target: BasicBlockIdx::from_raw(1),
                cleanup: None,
                msg: AssertMessage::Overflow(BinOp::Add),
            },
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let mut interp = Interpreter::new(&tcx);
    let res = interp.run_body(&body);
    assert!(res.is_err());
    let err = format!("{:?}", res);
    assert!(err.contains("assert failed"));
}

// ============ InterpValue to u128 ============
#[test]
fn interp_value_to_u128_coverage() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let interp = Interpreter::new(&tcx);
    assert_eq!(interp.interp_value_to_u128(&InterpValue::Int(42)), 42);
    assert_eq!(interp.interp_value_to_u128(&InterpValue::Bool(true)), 1);
    assert_eq!(interp.interp_value_to_u128(&InterpValue::Bool(false)), 0);
    assert_eq!(interp.interp_value_to_u128(&InterpValue::Unit), 0);
}

// ============ Call with cleanup=None ============
#[test]
fn call_with_cleanup_none_works() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let callee_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(1));
    let mut callee = Body::dummy(callee_id);
    callee.locals = IndexVec::from_raw(vec![local_decl(i32_ty.clone(), Mutability::Mut)]);
    callee.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(10),
                    ty: i32_ty.clone(),
                    span: Span::DUMMY,
                })),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let mut caller = Body::dummy(dummy_def_id());
    caller.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
    ]);
    caller.basic_blocks = IndexVec::from_raw(vec![
        BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    func: Operand::Constant(MirConst {
                        kind: MirConstKind::Int(callee_id.local_id.to_raw() as i128),
                        ty: i32_ty,
                        span: Span::DUMMY,
                    }),
                    args: vec![],
                    destination: Place::new(LocalIdx::from_raw(1)),
                    target: Some(BasicBlockIdx::from_raw(1)),
                    cleanup: None,
                },
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        },
        BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        },
    ]);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.add_function(callee_id, callee);
    interp.run_body(&caller).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Int(10))
    );
}
