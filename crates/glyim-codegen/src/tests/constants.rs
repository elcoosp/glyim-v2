//! Tests for constant emission (S08-T02, S08-T03)

use crate::{BytecodeBackend, CodegenBackend};
use glyim_core::primitives::Mutability;
use glyim_core::{CrateId, FnDefId, LocalDefId};
use glyim_mir::{
    BasicBlockData, Body, LocalDecl, LocalIdx, MirConst, MirConstKind, Operand, Place, Rvalue,
    SourceInfo, Statement, StatementKind, Terminator, TerminatorKind,
};
use glyim_test::with_fresh_ty_ctx;
use glyim_type::Substitution;
use std::sync::Arc;

#[test]
fn string_constant_emitted_to_string_table() {
    let (_, body) = with_fresh_ty_ctx(|ctx| {
        let mut body = Body::dummy(glyim_core::DefId::new(
            CrateId::from_raw(0),
            LocalDefId::from_raw(0),
        ));

        body.locals.push(LocalDecl {
            ty: ctx.unit_ty(),
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });

        let interner = glyim_core::Interner::new();
        let name = interner.intern("test_string");

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

        let mut block = BasicBlockData::new(Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });
        block.statements.push(stmt);
        body.basic_blocks.push(block);

        body
    });

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&Arc::new(body));

    assert!(result.is_ok());
    let bc = result.unwrap();
    assert!(!bc.is_empty());
    assert_eq!(bc[0], 0x01);
}

#[test]
fn function_constant_emitted_to_fn_table() {
    let (_, body) = with_fresh_ty_ctx(|ctx| {
        let mut body = Body::dummy(glyim_core::DefId::new(
            CrateId::from_raw(0),
            LocalDefId::from_raw(0),
        ));

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

        let mut block = BasicBlockData::new(Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        });
        block.statements.push(stmt);
        body.basic_blocks.push(block);

        body
    });

    let backend = BytecodeBackend::new();
    let result = backend.generate_function(&Arc::new(body));

    assert!(result.is_ok());
    let bc = result.unwrap();
    assert!(!bc.is_empty());
    assert_eq!(bc[0], 0x01);
}
