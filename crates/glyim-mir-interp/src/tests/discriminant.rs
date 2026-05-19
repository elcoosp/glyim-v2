use glyim_core::{CrateId, DefId, LocalDefId, Mutability, IndexVec};
use glyim_mir::*;
use glyim_type::Ty;
use crate::{Interpreter, InterpValue, InterpError};

/// Helper: build a minimal MIR Body with one basic block that assigns the
/// discriminant of a local to the return place, then returns.
fn build_discriminant_body(variant_count: usize, target_variant: usize) -> Body {
    let crate_id = CrateId::from_raw(0);
    let local_def_id = LocalDefId::from_raw(0);
    let owner = DefId::new(crate_id, local_def_id);

    // locals: 0 = return place, 1 = the enum value
    let num_locals = 2;
    let mut locals = IndexVec::with_capacity(num_locals);
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });

    let enum_place = Place::new(LocalIdx::from_raw(1));
    let return_place = Place::new(LocalIdx::from_raw(0));

    let discr_rvalue = Rvalue::Discriminant(enum_place);

    let stmt = Statement {
        kind: StatementKind::Assign(return_place.clone(), discr_rvalue),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    let terminator = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![stmt],
        terminator,
        is_cleanup: false,
    });

    Body {
        owner,
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: glyim_span::Span::DUMMY,
        var_debug_info: vec![],
    }
}

/// S19-T01: Discriminant of enum returns correct variant index
///
/// We construct an interpreter, set local 1 to an Aggregate whose first
/// element is the discriminant value (matching the convention in
/// eval_rvalue's Discriminant branch), then run the body and check
/// that the return place contains the correct variant index.
#[test]
fn discriminant_returns_variant_index() {
    let ctx = glyim_test::test_frozen_ty_ctx();
    let mut interp = Interpreter::new(&ctx);

    let body = build_discriminant_body(3, 2);

    // We need to pre-initialize local 1 with an aggregate that encodes variant 2.
    // The current Discriminant implementation reads fields[0] from an Aggregate.
    // So we construct: Aggregate([Int(2), Unit, Unit]) to represent variant 2.
    let variant_val = InterpValue::Aggregate(vec![
        InterpValue::Int(2),  // discriminant
        InterpValue::Unit,    // payload field 0
        InterpValue::Unit,    // payload field 1
    ]);

    // We cannot set locals directly before run_body. Instead, we need a body
    // that first writes the aggregate to local 1, then reads discriminant.
    // Let's build that body instead.
    let crate_id = CrateId::from_raw(0);
    let local_def_id = LocalDefId::from_raw(0);
    let owner = DefId::new(crate_id, local_def_id);

    let mut locals = IndexVec::with_capacity(3);
    // local 0: return place (i32)
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    // local 1: the enum value
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    // local 2: discriminant result
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });

    let enum_place = Place::new(LocalIdx::from_raw(1));
    let return_place = Place::new(LocalIdx::from_raw(0));

    // Statement 1: Assign local 1 = Aggregate([Int(2), Unit, Unit])
    let agg_stmt = Statement {
        kind: StatementKind::Assign(
            enum_place.clone(),
            Rvalue::Aggregate(
                AggregateKind::Tuple,
                vec![
                    Operand::Constant(MirConst {
                        kind: MirConstKind::Int(2),
                        ty: Ty::UNIT,
                        span: glyim_span::Span::DUMMY,
                    }),
                    Operand::Constant(MirConst {
                        kind: MirConstKind::Unit,
                        ty: Ty::UNIT,
                        span: glyim_span::Span::DUMMY,
                    }),
                    Operand::Constant(MirConst {
                        kind: MirConstKind::Unit,
                        ty: Ty::UNIT,
                        span: glyim_span::Span::DUMMY,
                    }),
                ],
            ),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    // Statement 2: Assign local 0 = Discriminant(local 1)
    let discr_stmt = Statement {
        kind: StatementKind::Assign(
            return_place.clone(),
            Rvalue::Discriminant(enum_place),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    let terminator = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![agg_stmt, discr_stmt],
        terminator,
        is_cleanup: false,
    });

    let body = Body {
        owner,
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: glyim_span::Span::DUMMY,
        var_debug_info: vec![],
    };

    let result = interp.run_body(&body);
    assert!(result.is_ok(), "run_body failed: {:?}", result);

    let ret = interp.get_local_value(LocalIdx::from_raw(0));
    assert!(ret.is_some(), "return place not set");
    assert_eq!(ret.unwrap(), &InterpValue::Int(2), "discriminant should be variant index 2");
}

/// S19-T01b: Discriminant of unit-like aggregate returns 0
#[test]
fn discriminant_empty_aggregate_returns_zero() {
    let ctx = glyim_test::test_frozen_ty_ctx();
    let mut interp = Interpreter::new(&ctx);

    let crate_id = CrateId::from_raw(0);
    let local_def_id = LocalDefId::from_raw(0);
    let owner = DefId::new(crate_id, local_def_id);

    let mut locals = IndexVec::with_capacity(2);
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });

    let enum_place = Place::new(LocalIdx::from_raw(1));
    let return_place = Place::new(LocalIdx::from_raw(0));

    let agg_stmt = Statement {
        kind: StatementKind::Assign(
            enum_place.clone(),
            Rvalue::Aggregate(AggregateKind::Tuple, vec![]),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    let discr_stmt = Statement {
        kind: StatementKind::Assign(
            return_place.clone(),
            Rvalue::Discriminant(enum_place),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    let terminator = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![agg_stmt, discr_stmt],
        terminator,
        is_cleanup: false,
    });

    let body = Body {
        owner,
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: glyim_span::Span::DUMMY,
        var_debug_info: vec![],
    };

    let result = interp.run_body(&body);
    assert!(result.is_ok(), "run_body failed: {:?}", result);

    let ret = interp.get_local_value(LocalIdx::from_raw(0));
    assert!(ret.is_some(), "return place not set");
    assert_eq!(ret.unwrap(), &InterpValue::Int(0), "empty aggregate discriminant should be 0");
}

/// S19-T01c: Discriminant on non-aggregate returns error
#[test]
fn discriminant_non_aggregate_errors() {
    let ctx = glyim_test::test_frozen_ty_ctx();
    let mut interp = Interpreter::new(&ctx);

    let crate_id = CrateId::from_raw(0);
    let local_def_id = LocalDefId::from_raw(0);
    let owner = DefId::new(crate_id, local_def_id);

    let mut locals = IndexVec::with_capacity(2);
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });

    let enum_place = Place::new(LocalIdx::from_raw(1));
    let return_place = Place::new(LocalIdx::from_raw(0));

    // Assign local 1 = Int(42) (not an aggregate)
    let int_stmt = Statement {
        kind: StatementKind::Assign(
            enum_place.clone(),
            Rvalue::Use(Operand::Constant(MirConst {
                kind: MirConstKind::Int(42),
                ty: Ty::UNIT,
                span: glyim_span::Span::DUMMY,
            })),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    let discr_stmt = Statement {
        kind: StatementKind::Assign(
            return_place.clone(),
            Rvalue::Discriminant(enum_place),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    let terminator = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    let mut basic_blocks = IndexVec::new();
    basic_blocks.push(BasicBlockData {
        statements: vec![int_stmt, discr_stmt],
        terminator,
        is_cleanup: false,
    });

    let body = Body {
        owner,
        basic_blocks,
        locals,
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: glyim_span::Span::DUMMY,
        var_debug_info: vec![],
    };

    let result = interp.run_body(&body);
    assert!(result.is_err(), "discriminant on non-aggregate should error");
    match result.unwrap_err() {
        InterpError::Panic(msg) => assert!(msg.contains("non-aggregate"), "unexpected panic: {}", msg),
        other => panic!("expected Panic error, got: {:?}", other),
    }
}
