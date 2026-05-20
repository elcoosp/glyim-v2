//! Tests for constant emission (S08-T02, S08-T03)

use glyim_test::test_frozen_ty_ctx;
use glyim_core::{FnDefId, CrateId, LocalDefId, Name};
use glyim_type::Substitution;
use glyim_mir::{MirConst, MirConstKind, Body, BasicBlockData, Terminator, TerminatorKind, SourceInfo, Statement, StatementKind, Rvalue, Operand, LocalDecl, Place, LocalIdx};
use glyim_core::primitives::Mutability;
use crate::{BytecodeBackend, CodegenBackend};
use std::sync::Arc;

#[test]
fn string_constant_emitted_to_string_table() {
    let ctx = test_frozen_ty_ctx();
    let mut body = Body::dummy(glyim_core::DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    body.locals.push(LocalDecl {
        ty: ctx.unit_ty(),
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    let name = Name::from_symbol(glyim_core::Interner::default().intern("test_string"));
    let mir_const = MirConst {
        kind: MirConstKind::String(name),
        ty: ctx.unit_ty(),
        span: glyim_span::Span::DUMMY,
    };
    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(0)),
            Rvalue::Use(Operand::Constant(mir_const)),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    let block = BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    body.basic_blocks.push(block);
    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&Arc::new(body));
    assert!(result.is_ok());
    let bc = result.unwrap();
    assert!(!bc.is_empty());
    assert_eq!(bc[0], 0x01);
}

#[test]
fn function_constant_emitted_to_fn_table() {
    let ctx = test_frozen_ty_ctx();
    let mut body = Body::dummy(glyim_core::DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    body.locals.push(LocalDecl {
        ty: ctx.unit_ty(),
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    let def_id = FnDefId::from_raw(42);
    let substs = Substitution::empty();
    let mir_const = MirConst {
        kind: MirConstKind::Fn(def_id, substs),
        ty: ctx.unit_ty(),
        span: glyim_span::Span::DUMMY,
    };
    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(0)),
            Rvalue::Use(Operand::Constant(mir_const)),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    let block = BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    body.basic_blocks.push(block);
    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&Arc::new(body));
    assert!(result.is_ok());
    let bc = result.unwrap();
    assert!(!bc.is_empty());
    assert_eq!(bc[0], 0x01);
}
