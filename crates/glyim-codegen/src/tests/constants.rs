//! Tests for constant emission (S08-T02, S08-T03)

use crate::{BytecodeBackend, CodegenBackend};
use glyim_core::primitives::Mutability;
use glyim_core::{CrateId, FnDefId, Interner, LocalDefId};
use glyim_mir::{
    BasicBlockIdx, Body, LocalDecl, LocalIdx, MirConst, MirConstKind, Operand, Place, Rvalue,
    SourceInfo, Statement, StatementKind, Terminator, TerminatorKind,
};
use glyim_type::{Substitution, TyCtxMut};
use std::sync::Arc;

#[test]
fn string_constant_emitted_to_string_table() {
    let interner = Interner::default();
    let ctx_mut = TyCtxMut::new(interner.clone());

    let mut body = Body::dummy(glyim_core::DefId::new(
        CrateId::from_raw(0),
        LocalDefId::from_raw(0),
    ));

    body.locals.push(LocalDecl {
        ty: ctx_mut.unit_ty(),
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });

    let name = interner.intern("test_string");

    let mir_const = MirConst {
        kind: MirConstKind::String(name),
        ty: ctx_mut.unit_ty(),
        span: glyim_span::Span::DUMMY,
    };

    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(0)),
            Rvalue::Use(Operand::Constant(mir_const)),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    body.basic_blocks[BasicBlockIdx::from_raw(0)]
        .statements
        .push(stmt);
    body.basic_blocks[BasicBlockIdx::from_raw(0)].terminator = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    let ctx = ctx_mut.freeze();
    let backend = BytecodeBackend::with_ty_ctx(Arc::new(ctx), glyim_core::TargetInfo::default());
    let result = backend.generate_function(&Arc::new(body));

    assert!(result.is_ok());
    let bc = result.unwrap();
    assert!(!bc.is_empty());
    assert_eq!(bc[0], 0x01);
}

#[test]
fn function_constant_emitted_to_fn_table() {
    let interner = Interner::default();
    let ctx_mut = TyCtxMut::new(interner.clone());

    let mut body = Body::dummy(glyim_core::DefId::new(
        CrateId::from_raw(0),
        LocalDefId::from_raw(0),
    ));

    body.locals.push(LocalDecl {
        ty: ctx_mut.unit_ty(),
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    });

    let def_id = FnDefId::from_raw(42);
    let substs = Substitution::empty();

    let mir_const = MirConst {
        kind: MirConstKind::Fn(def_id, substs),
        ty: ctx_mut.unit_ty(),
        span: glyim_span::Span::DUMMY,
    };

    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(0)),
            Rvalue::Use(Operand::Constant(mir_const)),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    body.basic_blocks[BasicBlockIdx::from_raw(0)]
        .statements
        .push(stmt);
    body.basic_blocks[BasicBlockIdx::from_raw(0)].terminator = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };

    let ctx = ctx_mut.freeze();
    let backend = BytecodeBackend::with_ty_ctx(Arc::new(ctx), glyim_core::TargetInfo::default());
    let result = backend.generate_function(&Arc::new(body));

    assert!(result.is_ok());
    let bc = result.unwrap();
    assert!(!bc.is_empty());
    assert_eq!(bc[0], 0x01);
}
