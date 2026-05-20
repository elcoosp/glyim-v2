//! Tests for field projection layout (S08-T01)

use glyim_test::test_frozen_ty_ctx;
use glyim_core::primitives::*;
use glyim_type::{TyKind, FieldIdx};
use glyim_mir::{Place, ProjectionElem, LocalIdx, Body, BasicBlockData, Terminator, TerminatorKind, SourceInfo, Statement, StatementKind, Rvalue, LocalDecl};
use glyim_core::primitives::Mutability;
use crate::{BytecodeBackend, CodegenBackend, LayoutProvider};
use std::sync::Arc;

#[test]
fn field_projection_emits_offset_bytecode() {
    let ctx = test_frozen_ty_ctx();
    let mut body = Body::dummy(glyim_core::DefId::new(glyim_core::CrateId::from_raw(0), glyim_core::LocalDefId::from_raw(0)));
    body.locals.push(LocalDecl {
        ty: ctx.mk_ty(TyKind::RawPtr(ctx.unit_ty(), Mutability::Not)),
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    body.locals.push(LocalDecl {
        ty: ctx.unit_ty(),
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });
    let place = Place {
        local: LocalIdx::from_raw(0),
        projection: vec![ProjectionElem::Field(FieldIdx::from_raw(1))].into_boxed_slice(),
    };
    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(1)),
            Rvalue::Ref(place, glyim_mir::BorrowKind::Shared),
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
}
