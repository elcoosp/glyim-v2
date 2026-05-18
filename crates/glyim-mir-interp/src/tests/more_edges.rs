use crate::*;
use glyim_core::{BinOp, CrateId, DefId, IndexVec, IntTy, LocalDefId, Mutability, UnOp};
use glyim_mir::{
    BasicBlockData, BasicBlockIdx, Body, LocalIdx, MirConst, MirConstKind, Operand, Place, Rvalue,
    Statement, StatementKind, SwitchTargets, Terminator, TerminatorKind,
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

// ============ Drop with cleanup=None on non-trivial place ============
#[test]
fn drop_with_cleanup_none_advances() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut body = Body::dummy(dummy_def_id());
    let local_to_drop = LocalIdx::from_raw(1);
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(Ty::BOOL, Mutability::Mut),
    ]);
    // Assign true then drop, advance to BB1 which returns
    body.basic_blocks = IndexVec::from_raw(vec![
        BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::new(local_to_drop),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Bool(false),
                        ty: Ty::BOOL,
                        span: Span::DUMMY,
                    })),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            }],
            terminator: Terminator {
                kind: TerminatorKind::Drop {
                    place: Place::new(local_to_drop),
                    target: BasicBlockIdx::from_raw(1),
                    cleanup: None,
                },
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        },
        BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
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
        },
    ]);
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap();
}

// ============ SwitchInt with empty branch list ============
#[test]
fn switch_int_empty_branches_goes_to_otherwise() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    let discr_local = LocalIdx::from_raw(1);
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty, Mutability::Not),
    ]);
    // Set discr = 0, empty branches, otherwise -> BB1 (Return)
    body.basic_blocks = IndexVec::from_raw(vec![
        BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::new(discr_local),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(0),
                        ty: i32_ty,
                        span: Span::DUMMY,
                    })),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            }],
            terminator: Terminator {
                kind: TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Place::new(discr_local)),
                    switch_ty: i32_ty,
                    targets: SwitchTargets::new(
                        vec![].into_boxed_slice(),
                        BasicBlockIdx::from_raw(1),
                    ),
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
    interp.run_body(&body).unwrap();
}

// ============ Call with non-Int constant (Bool) as callee ============
#[test]
fn call_with_bool_callee_panics() {
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
    let mut interp = Interpreter::new(&tcx);
    let res = interp.run_body(&body);
    assert!(
        res.is_err(),
        "Expected error when callee is not a function reference"
    );
}

// ============ FnPtrToPtr cast stub ============

#[test]
fn fn_ptr_to_ptr_cast_returns_success() {
    let mut tcx = test_ty_ctx();
    let i32_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let ptr_ty = tcx.mk_ty(TyKind::RawPtr(i32_ty, Mutability::Not));
    let const_val = MirConst {
        kind: MirConstKind::Int(42),
        ty: i32_ty,
        span: Span::DUMMY,
    };
    let operand = Operand::Constant(const_val);
    let cast_rvalue = Rvalue::Cast(CastKind::PtrToPtr, operand, ptr_ty);
    let assign_stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(0)), cast_rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![local_decl(ptr_ty, Mutability::Mut)]);
    let bb_data = BasicBlockData {
        statements: vec![assign_stmt],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    };
    body.basic_blocks.push(bb_data);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.locals.resize(body.locals.len(), None);
    let res = interp.run_body(&body);
    assert!(res.is_ok());
}


// ============ Multiple return paths through branching ============
#[test]
fn multi_path_return_correct_value() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut body = Body::dummy(dummy_def_id());
    let discr_local = LocalIdx::from_raw(1);
    let result_local = LocalIdx::from_raw(2);
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(Ty::BOOL, Mutability::Not),
        local_decl(Ty::BOOL, Mutability::Mut),
    ]);
    // Set discr = true, then switch: true -> BB1 assigns 10, false -> BB2 assigns 20, otherwise -> BB3 assigns 30
    body.basic_blocks = IndexVec::from_raw(vec![
        BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::new(discr_local),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Bool(true),
                        ty: Ty::BOOL,
                        span: Span::DUMMY,
                    })),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            }],
            terminator: Terminator {
                kind: TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Place::new(discr_local)),
                    switch_ty: Ty::BOOL,
                    targets: SwitchTargets::new(
                        vec![(1u128, BasicBlockIdx::from_raw(1))].into_boxed_slice(),
                        BasicBlockIdx::from_raw(2),
                    ),
                },
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        },
        BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::new(result_local),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(10),
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
        },
        BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::new(result_local),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(20),
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
        },
    ]);
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(2)),
        Some(&InterpValue::Int(10))
    );
}

// ============ Overwrite function in table ============
#[test]
fn overwrite_function_in_table_uses_latest() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let callee_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(1));

    // First version: returns 10
    let mut callee_v1 = Body::dummy(callee_id);
    callee_v1.locals = IndexVec::from_raw(vec![local_decl(i32_ty, Mutability::Mut)]);
    callee_v1.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(10),
                    ty: i32_ty,
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

    // Second version: returns 99
    let mut callee_v2 = Body::dummy(callee_id);
    callee_v2.locals = IndexVec::from_raw(vec![local_decl(i32_ty, Mutability::Mut)]);
    callee_v2.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(99),
                    ty: i32_ty,
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

    // Caller: calls callee_id and stores in local1
    let mut caller = Body::dummy(dummy_def_id());
    caller.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty, Mutability::Mut),
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
    interp.add_function(callee_id, callee_v1);
    interp.add_function(callee_id, callee_v2); // overwrite
    interp.run_body(&caller).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Int(99))
    );
}

// ============ Step count edge: body with many statements in one block ============
#[test]
fn step_count_tracks_per_bb_not_per_statement() {
    // Each basic block is 1 step, regardless of how many statements
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(Ty::BOOL, Mutability::Mut),
    ]);
    // One block with 5 Nop statements, then Return
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![
            Statement {
                kind: StatementKind::Nop,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Nop,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Nop,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Nop,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Nop,
                source_info: SourceInfo::new(Span::DUMMY),
            },
        ],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let mut interp = Interpreter::new(&tcx).with_step_limit(1);
    let res = interp.run_body(&body);
    assert!(res.is_ok()); // 1 step covers entire block
}

// ============ StorageLive after assignment (re-init) ============
#[test]
fn storage_live_after_assign_reinitializes() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    let local = LocalIdx::from_raw(1);
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty, Mutability::Mut),
    ]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![
            Statement {
                kind: StatementKind::Assign(
                    Place::new(local),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(42),
                        ty: i32_ty,
                        span: Span::DUMMY,
                    })),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::StorageLive(local),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::new(local),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(99),
                        ty: i32_ty,
                        span: Span::DUMMY,
                    })),
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
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Int(99))
    );
}

// ============ Negative operands in comparisons ============
#[test]
fn negative_comparisons() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut interp = Interpreter::new(&tcx);
    // -5 < -3 should be true
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(Ty::BOOL, Mutability::Mut),
    ]);
    let c1 = MirConst {
        kind: MirConstKind::Int(-5),
        ty: i32_ty,
        span: Span::DUMMY,
    };
    let c2 = MirConst {
        kind: MirConstKind::Int(-3),
        ty: i32_ty,
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::BinaryOp(
                    BinOp::Lt,
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
    interp.run_body(&body).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Bool(true))
    );
}

// ============ Unary Neg on negative ============
#[test]
fn unary_neg_on_negative_makes_positive() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty, Mutability::Mut),
    ]);
    let c = MirConst {
        kind: MirConstKind::Int(-42),
        ty: i32_ty,
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
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Int(42))
    );
}

// ============ Nested calls restoring local_decls ============
#[test]
fn nested_calls_restore_local_decls() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));

    // Inner callee: returns 5
    let inner_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(2));
    let mut inner = Body::dummy(inner_id);
    inner.locals = IndexVec::from_raw(vec![local_decl(i32_ty, Mutability::Mut)]);
    inner.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(5),
                    ty: i32_ty,
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

    // Outer callee: calls inner, adds 10, returns
    let outer_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(1));
    let mut outer = Body::dummy(outer_id);
    outer.locals = IndexVec::from_raw(vec![
        local_decl(i32_ty, Mutability::Mut),
        local_decl(i32_ty, Mutability::Mut),
    ]);
    outer.basic_blocks = IndexVec::from_raw(vec![
        BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    func: Operand::Constant(MirConst {
                        kind: MirConstKind::Int(2),
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
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::BinaryOp(
                        BinOp::Add,
                        Box::new((
                            Operand::Copy(Place::new(LocalIdx::from_raw(1))),
                            Operand::Constant(MirConst {
                                kind: MirConstKind::Int(10),
                                ty: i32_ty,
                                span: Span::DUMMY,
                            }),
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

    // Caller: calls outer and stores result
    let mut caller = Body::dummy(dummy_def_id());
    caller.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty, Mutability::Mut),
    ]);
    caller.basic_blocks = IndexVec::from_raw(vec![
        BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Call {
                    func: Operand::Constant(MirConst {
                        kind: MirConstKind::Int(1),
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
    interp.add_function(inner_id, inner);
    interp.add_function(outer_id, outer);
    interp.run_body(&caller).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Int(15)) // 5 + 10
    );
}

// ============ Comparison between different negative values ============
#[test]
fn negative_gt_comparison() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut interp = Interpreter::new(&tcx);
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(Ty::BOOL, Mutability::Mut),
    ]);
    let c1 = MirConst {
        kind: MirConstKind::Int(-3),
        ty: i32_ty,
        span: Span::DUMMY,
    };
    let c2 = MirConst {
        kind: MirConstKind::Int(-5),
        ty: i32_ty,
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::BinaryOp(
                    BinOp::Gt,
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
    interp.run_body(&body).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Bool(true)) // -3 > -5
    );
}
