use crate::*;
use glyim_core::{BinOp, CrateId, DefId, IndexVec, IntTy, LocalDefId, Mutability, UnOp};
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

// ----- Helper: run a simple body with one assignment and return -----
fn run_single_assign(body: Body, tcx: &TyCtx) -> InterpResult<()> {
    let mut interp = Interpreter::new(tcx);
    interp.run_body(&body)
}

// ============ Function not found ============
#[test]
fn call_missing_function_panics() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![local_decl(Ty::UNIT, Mutability::Mut)]);
    let missing_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(99));
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Call {
                func: Operand::Constant(MirConst {
                    kind: MirConstKind::Int(missing_id.local_id.to_raw() as i128),
                    ty: Ty::UNIT,
                    span: Span::DUMMY,
                }),
                args: vec![],
                destination: Place::new(LocalIdx::from_raw(0)),
                target: None,
                cleanup: None,
            },
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let res = run_single_assign(body, &tcx);
    assert!(res.is_err());
    assert!(format!("{:?}", res).contains("not found"));
}

// ============ Callee non-int constant ============
#[test]
fn call_with_non_int_callee_panics() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![local_decl(Ty::UNIT, Mutability::Mut)]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Call {
                func: Operand::Constant(MirConst {
                    kind: MirConstKind::Bool(true),
                    ty: Ty::BOOL,
                    span: Span::DUMMY,
                }),
                args: vec![],
                destination: Place::new(LocalIdx::from_raw(0)),
                target: None,
                cleanup: None,
            },
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let res = run_single_assign(body, &tcx);
    assert!(res.is_err());
    assert!(format!("{:?}", res).contains("constant int encoding DefId"));
}

// ============ StorageDead then read ============
#[test]
fn storage_dead_then_read_panics() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    let local = LocalIdx::from_raw(1);
    let read_local = LocalIdx::from_raw(2);
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
    ]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![
            Statement {
                kind: StatementKind::StorageLive(local),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::new(local),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(42),
                        ty: i32_ty.clone(),
                        span: Span::DUMMY,
                    })),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::StorageDead(local),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::new(read_local),
                    Rvalue::Use(Operand::Copy(Place::new(local))),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
        ],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let tcx = tcx_mut.freeze();
    let res = run_single_assign(body, &tcx);
    assert!(res.is_err());
    assert!(format!("{:?}", res).contains("uninitialized"));
}

// ============ Aggregate Array stub ============
#[test]
fn aggregate_array_returns_error() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
    ]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::Aggregate(AggregateKind::Array(i32_ty), vec![]),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let tcx = tcx_mut.freeze();
    let res = run_single_assign(body, &tcx);
    assert!(res.is_err());
    assert!(format!("{:?}", res).contains("Aggregate"));
}

// ============ Aggregate Closure stub ============
#[test]
fn aggregate_closure_returns_error() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let empty_subst = tcx_mut.intern_substitution(vec![]);
    let tcx = tcx_mut.freeze();
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
    ]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::Aggregate(
                    AggregateKind::Closure(glyim_core::ClosureId::from_raw(0), empty_subst),
                    vec![],
                ),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let res = run_single_assign(body, &tcx);
    assert!(res.is_err());
    assert!(format!("{:?}", res).contains("Aggregate"));
}

// ============ Cast IntToFloat stub ============
#[test]
fn int_to_float_cast_returns_error() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let f32_ty = tcx_mut.mk_ty(TyKind::Float(glyim_core::FloatTy::F32));
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
        local_decl(f32_ty.clone(), Mutability::Mut),
    ]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![
            Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(42),
                        ty: i32_ty,
                        span: Span::DUMMY,
                    })),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(2)),
                    Rvalue::Cast(
                        CastKind::IntToFloat,
                        Operand::Copy(Place::new(LocalIdx::from_raw(1))),
                        f32_ty,
                    ),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
        ],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let tcx = tcx_mut.freeze();
    let res = run_single_assign(body, &tcx);
    assert!(res.is_err());
    assert!(format!("{:?}", res).contains("Cast"));
}

// ============ BinOp And on Ints ============
#[test]
fn binop_and_on_ints_returns_error() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
    ]);
    let c1 = MirConst {
        kind: MirConstKind::Int(1),
        ty: i32_ty.clone(),
        span: Span::DUMMY,
    };
    let c2 = MirConst {
        kind: MirConstKind::Int(1),
        ty: i32_ty.clone(),
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::BinaryOp(
                    BinOp::And,
                    Box::new((Operand::Constant(c1), Operand::Constant(c2))),
                ),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let tcx = tcx_mut.freeze();
    let res = run_single_assign(body, &tcx);
    assert!(res.is_err());
    assert!(format!("{:?}", res).contains("unsupported"));
}

// ============ Unary Not on Int ============
#[test]
fn unary_not_on_int_returns_error() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
    ]);
    let c = MirConst {
        kind: MirConstKind::Int(1),
        ty: i32_ty,
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::UnaryOp(UnOp::Not, Operand::Constant(c)),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let tcx = tcx_mut.freeze();
    let res = run_single_assign(body, &tcx);
    assert!(res.is_err());
    assert!(format!("{:?}", res).contains("unsupported unary op"));
}

// ============ Unary Neg on Bool ============
#[test]
fn unary_neg_on_bool_returns_error() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(Ty::BOOL, Mutability::Mut),
    ]);
    let c = MirConst {
        kind: MirConstKind::Bool(true),
        ty: Ty::BOOL,
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::UnaryOp(UnOp::Neg, Operand::Constant(c)),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let res = run_single_assign(body, &tcx);
    assert!(res.is_err());
    assert!(format!("{:?}", res).contains("unsupported unary op"));
}

// ============ Write to out-of-bounds local ============
#[test]
fn write_out_of_bounds_local_panics() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![local_decl(Ty::UNIT, Mutability::Mut)]);
    let oob_local = LocalIdx::from_raw(99);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(oob_local),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Unit,
                    ty: Ty::UNIT,
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
    let res = run_single_assign(body, &tcx);
    assert!(res.is_err());
    assert!(format!("{:?}", res).contains("out of bounds"));
}

// ============ Multiple sequential calls ============
#[test]
fn multiple_sequential_calls_accumulate() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));

    // Callee1: returns 10
    let callee1_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(1));
    let mut callee1 = Body::dummy(callee1_id);
    callee1.locals = IndexVec::from_raw(vec![local_decl(i32_ty.clone(), Mutability::Mut)]);
    callee1.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
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

    // Callee2: returns 20
    let callee2_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(2));
    let mut callee2 = Body::dummy(callee2_id);
    callee2.locals = IndexVec::from_raw(vec![local_decl(i32_ty.clone(), Mutability::Mut)]);
    callee2.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(20),
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

    // Caller: calls callee1, stores result in local1, then calls callee2, adds to local1, final result in local2
    let mut caller = Body::dummy(dummy_def_id());
    let tmp1 = LocalIdx::from_raw(1);
    let tmp2 = LocalIdx::from_raw(2);
    let sum = LocalIdx::from_raw(3);
    caller.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
    ]);
    caller.basic_blocks = IndexVec::from_raw(vec![
        // BB0: call callee1 -> tmp1, goto BB1
        BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    func: Operand::Constant(MirConst {
                        kind: MirConstKind::Int(1),
                        ty: i32_ty.clone(),
                        span: Span::DUMMY,
                    }),
                    args: vec![],
                    destination: Place::new(tmp1),
                    target: Some(BasicBlockIdx::from_raw(1)),
                    cleanup: None,
                },
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        },
        // BB1: call callee2 -> tmp2, goto BB2
        BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    func: Operand::Constant(MirConst {
                        kind: MirConstKind::Int(2),
                        ty: i32_ty.clone(),
                        span: Span::DUMMY,
                    }),
                    args: vec![],
                    destination: Place::new(tmp2),
                    target: Some(BasicBlockIdx::from_raw(2)),
                    cleanup: None,
                },
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        },
        // BB2: sum = tmp1 + tmp2; return
        BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::new(sum),
                    Rvalue::BinaryOp(
                        BinOp::Add,
                        Box::new((
                            Operand::Copy(Place::new(tmp1)),
                            Operand::Copy(Place::new(tmp2)),
                        )),
                    ),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            }],
            terminator: Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        },
    ]);

    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.add_function(callee1_id, callee1);
    interp.add_function(callee2_id, callee2);
    interp.run_body(&caller).unwrap();
    assert_eq!(interp.get_local_value(sum), Some(&InterpValue::Int(30)));
}

// ============ Step count resets between run_body calls ============
#[test]
fn step_count_resets_between_runs() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let body = {
        let mut b = Body::dummy(dummy_def_id());
        b.locals = IndexVec::from_raw(vec![local_decl(Ty::UNIT, Mutability::Mut)]);
        b.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Return,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        }]);
        b
    };
    let mut interp = Interpreter::new(&tcx).with_step_limit(0);
    // First run will time out (step 0)
    let res1 = interp.run_body(&body);
    assert_eq!(res1, Err(InterpError::TimedOut));
    // Change step limit and run again
    interp = interp.with_step_limit(10);
    let res2 = interp.run_body(&body);
    assert!(res2.is_ok());
}

// ============ Move operand works like Copy ============
#[test]
fn move_operand_reads_value() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Mut),
    ]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![
            Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(42),
                        ty: i32_ty.clone(),
                        span: Span::DUMMY,
                    })),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(2)),
                    Rvalue::Use(Operand::Move(Place::new(LocalIdx::from_raw(1)))),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
        ],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(2)),
        Some(&InterpValue::Int(42))
    );
}

// ============ Nested Switch with int values ============
#[test]
fn switch_int_multiple_values() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    let discr = LocalIdx::from_raw(1);
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty.clone(), Mutability::Not),
    ]);
    // Set discr = 42
    body.basic_blocks = IndexVec::from_raw(vec![
        BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::new(discr),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(42),
                        ty: i32_ty,
                        span: Span::DUMMY,
                    })),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            }],
            terminator: Terminator {
                kind: TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Place::new(discr)),
                    switch_ty: i32_ty,
                    targets: SwitchTargets::new(
                        vec![
                            (10u128, BasicBlockIdx::from_raw(1)),
                            (42u128, BasicBlockIdx::from_raw(2)),
                            (100u128, BasicBlockIdx::from_raw(3)),
                        ]
                        .into_boxed_slice(),
                        BasicBlockIdx::from_raw(4), // otherwise
                    ),
                },
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        },
        BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Unreachable,
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
        BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Unreachable,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        },
        BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Unreachable,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        },
    ]);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap(); // reaches BB2 -> Return
}
