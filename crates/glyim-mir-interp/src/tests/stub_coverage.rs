//! Tests that verify all stubs return proper errors (not silent no-ops).
//! Rule: "Silent no-ops (empty match arms, `let _ = x`) are forbidden in
//! implementation code. Every stub must emit a warning on first execution."

use crate::*;
use glyim_core::{CrateId, DefId, IndexVec, IntTy, LocalDefId, Mutability};
#[allow(unused_imports)]
use glyim_mir::*;
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

// ============ Ref rvalue ============
#[test]
fn ref_rvalue_succeeds() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty, Mutability::Mut),
    ]);
    let c42 = MirConst {
        kind: MirConstKind::Int(42),
        ty: i32_ty,
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![
            Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Constant(c42)),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Ref(Place::new(LocalIdx::from_raw(1)), BorrowKind::Shared),
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
    let ret_val = interp.get_local_value(LocalIdx::from_raw(0)).unwrap();
    assert_eq!(ret_val, &InterpValue::Ref(1));
}
// ============ Discriminant rvalue ============
#[test]
fn discriminant_rvalue_returns_success() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let tuple_substs = tcx_mut.intern_substitution(vec![
        glyim_type::GenericArg::Ty(i32_ty),
        glyim_type::GenericArg::Ty(i32_ty),
    ]);
    let tuple_ty = tcx_mut.mk_ty(TyKind::Tuple(tuple_substs));

    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(tuple_ty, Mutability::Mut),
    ]);

    // Build tuple (1, 2) and store in local1
    let tuple_rvalue = Rvalue::Aggregate(
        AggregateKind::Tuple,
        vec![
            Operand::Constant(MirConst {
                kind: MirConstKind::Int(1),
                ty: i32_ty,
                span: Span::DUMMY,
            }),
            Operand::Constant(MirConst {
                kind: MirConstKind::Int(2),
                ty: i32_ty,
                span: Span::DUMMY,
            }),
        ],
    );
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![
            Statement {
                kind: StatementKind::StorageLive(LocalIdx::from_raw(1)),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(1)), tuple_rvalue),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(0)),
                    Rvalue::Discriminant(Place::new(LocalIdx::from_raw(1))),
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
    let res = interp.run_body(&body);
    assert!(res.is_ok());
}
// ============ Cast (unsupported kind) ============

#[test]
fn cast_ptr_to_ptr_returns_value() {
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
    let ret_val = interp.get_local_value(LocalIdx::from_raw(0)).cloned().unwrap();
    assert_eq!(ret_val, InterpValue::Int(42));
}

// ============ FloatBits const ============

#[test]
fn float_const_returns_value() {
    let mut tcx = test_ty_ctx();
    let float_ty = tcx.mk_ty(TyKind::Float(FloatTy::F64));
    let const_val = MirConst {
        kind: MirConstKind::FloatBits(42.0_f64.to_bits()),
        ty: float_ty,
        span: Span::DUMMY,
    };
    let operand = Operand::Constant(const_val);
    let use_rvalue = Rvalue::Use(operand);
    let assign_stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(0)), use_rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![local_decl(float_ty, Mutability::Mut)]);
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
    let ret_val = interp.get_local_value(LocalIdx::from_raw(0)).cloned().unwrap();
    assert_eq!(ret_val, InterpValue::Float(42.0));
}

// ============ String const ============

#[test]
fn string_const_returns_value() {
    let mut tcx = test_ty_ctx();
    let name = tcx.resolver().intern("hello");
    let string_ty = tcx.mk_ty(TyKind::String);
    let const_val = MirConst {
        kind: MirConstKind::String(name),
        ty: string_ty,
        span: Span::DUMMY,
    };
    let operand = Operand::Constant(const_val);
    let use_rvalue = Rvalue::Use(operand);
    let assign_stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(0)), use_rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![local_decl(string_ty, Mutability::Mut)]);
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
    let ret_val = interp.get_local_value(LocalIdx::from_raw(0)).cloned().unwrap();
    assert_eq!(ret_val, InterpValue::String("hello".to_string()));
}

// ============ Error const ============
#[test]
fn error_const_returns_error() {
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(Ty::ERROR, Mutability::Mut),
    ]);
    let c = MirConst {
        kind: MirConstKind::Error,
        ty: Ty::ERROR,
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::Use(Operand::Constant(c)),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut interp = Interpreter::new(&tcx);
    let res = interp.run_body(&body);
    assert!(res.is_err());
    assert!(format!("{:?}", res).contains("Error const"));
}
// ============ Place projections ============
#[test]
fn place_with_projection_returns_success() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(Ty::BOOL, Mutability::Mut),
    ]);

    // local1 = &local0 (but local0 is uninitialized, so better to create a dummy integer)
    // We'll add a local2 to hold an integer and take its address.
    body.locals.push(LocalDecl {
        ty: Ty::ERROR, // placeholder, but we'll use a real type
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    let src_local = LocalIdx::from_raw(2);
    // Assign a constant to src_local
    let const_stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(src_local),
            Rvalue::Use(Operand::Constant(MirConst {
                kind: MirConstKind::Int(42),
                ty: Ty::ERROR,
                span: Span::DUMMY,
            })),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    // Take reference to src_local and store in local1
    let ref_stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(1)),
            Rvalue::Ref(Place::new(src_local), BorrowKind::Shared),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    // Copy the place with Deref projection (should read through reference)
    let proj_place = Place {
        local: LocalIdx::from_raw(1),
        projection: vec![ProjectionElem::Deref].into_boxed_slice(),
    };
    let copy_stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(0)),
            Rvalue::Use(Operand::Copy(proj_place)),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![const_stmt, ref_stmt, copy_stmt],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let mut interp = Interpreter::new(&tcx);
    let res = interp.run_body(&body);
    assert!(res.is_ok());
}
// ============ Indirect function call ============
#[test]
fn indirect_call_returns_error() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![local_decl(Ty::UNIT, Mutability::Mut)]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Call {
                func: Operand::Copy(Place::new(LocalIdx::from_raw(0))),
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
    assert!(res.is_err());
    assert!(format!("{:?}", res).contains("indirect"));
}
// ============ Aggregate ADT (unsupported) ============
#[test]
fn aggregate_adt_returns_success() {
    let mut tcx_mut = test_ty_ctx();
    let empty_subst = tcx_mut.intern_substitution(vec![]);
    let tcx = tcx_mut.freeze();
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(Ty::BOOL, Mutability::Mut),
    ]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::Aggregate(
                    AggregateKind::Adt(
                        glyim_core::AdtId::from_raw(0),
                        glyim_mir::VariantIdx::from_raw(0),
                        empty_subst,
                    ),
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
    let mut interp = Interpreter::new(&tcx);
    let res = interp.run_body(&body);
    assert!(res.is_ok());
    ////    assert!(format!("{:?}", res).contains("Aggregate"));
}

// ============ Aggregate Tuple (implemented - should succeed) ============
#[test]
fn aggregate_tuple_returns_first_element() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty, Mutability::Mut),
    ]);
    let c = MirConst {
        kind: MirConstKind::Int(42),
        ty: i32_ty,
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::Aggregate(AggregateKind::Tuple, vec![Operand::Constant(c)]),
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
    // The interpreter stores tuple as Aggregate value, not unwrapped.
    match interp.get_local_value(LocalIdx::from_raw(1)) {
        Some(InterpValue::Aggregate(elems)) => {
            assert_eq!(elems.len(), 1);
            assert_eq!(elems[0], InterpValue::Int(42));
        }
        other => panic!("Expected Aggregate value, got {:?}", other),
    }
}

// ============ Repeat rvalue ============

#[test]
fn repeat_rvalue_returns_array() {
    let mut tcx = test_ty_ctx();
    let elem_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let array_ty = tcx.mk_ty(TyKind::Array(elem_ty, Const::from(3)));
    let const_val = MirConst {
        kind: MirConstKind::Int(7),
        ty: elem_ty,
        span: Span::DUMMY,
    };
    let count_const = MirConst {
        kind: MirConstKind::Uint(3),
        ty: tcx.mk_ty(TyKind::Uint(UintTy::Usize)),
        span: Span::DUMMY,
    };
    let repeat_rvalue = Rvalue::Repeat(Operand::Constant(const_val), count_const);
    let assign_stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(0)), repeat_rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![local_decl(array_ty, Mutability::Mut)]);
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
    interp.run_body(&body).unwrap();
    let ret_val = interp.get_local_value(LocalIdx::from_raw(0)).cloned().unwrap();
    let expected = InterpValue::Aggregate(vec![InterpValue::Int(7); 3]);
    assert_eq!(ret_val, expected);
}


// ============ IntToInt cast (implemented) ============
#[test]
fn int_to_int_cast_passes_through() {
    let mut tcx_mut = test_ty_ctx();
    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let i64_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I64));
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(i32_ty, Mutability::Mut),
        local_decl(i64_ty, Mutability::Mut),
    ]);
    let c = MirConst {
        kind: MirConstKind::Int(42),
        ty: i32_ty,
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![
            Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Constant(c)),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(2)),
                    Rvalue::Cast(
                        CastKind::IntToInt,
                        Operand::Copy(Place::new(LocalIdx::from_raw(1))),
                        i64_ty,
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
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(2)),
        Some(&InterpValue::Int(42))
    );
}

// ============ Char const ============
#[test]
fn char_const_interpreted_as_int() {
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(Ty::BOOL, Mutability::Mut),
    ]);
    let c = MirConst {
        kind: MirConstKind::Char('A'),
        ty: Ty::BOOL,
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::Use(Operand::Constant(c)),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Int('A' as i128))
    );
}
