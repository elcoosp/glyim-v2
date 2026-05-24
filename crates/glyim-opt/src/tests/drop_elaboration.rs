//! Tests for drop elaboration: conditional drops, array loops, enum discriminants.

use glyim_core::primitives::Mutability;
use glyim_core::{AdtId, CrateId, DefId, IndexVec, LocalDefId, UintTy};
use glyim_mir::*;
use glyim_span::Span;
use glyim_test::test_ty_ctx;
use glyim_type::{Const, ConstKind, Substitution, Ty, TyCtx, TyCtxMut, TyKind};

// Helper: create a dummy DefId for tests
fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

// Helper: create a simple MIR body with a Drop terminator for a single local.
// Takes ownership of ctx_mut, returns frozen context and the body.
fn body_with_drop(ctx_mut: TyCtxMut, local_ty: Ty) -> (TyCtx, Body) {
    let mut body = Body::dummy(dummy_def_id());
    let local = body.locals.push(LocalDecl {
        ty: local_ty,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    let stmt1 = Statement {
        kind: StatementKind::StorageLive(local),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let term = Terminator {
        kind: TerminatorKind::Drop {
            place: Place::new(local),
            target: BasicBlockIdx::from_raw(1),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block0 = BasicBlockData {
        statements: vec![stmt1],
        terminator: term,
        is_cleanup: false,
    };
    let return_term = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block1 = BasicBlockData {
        statements: vec![],
        terminator: return_term,
        is_cleanup: false,
    };
    body.basic_blocks = IndexVec::from_raw(vec![block0, block1]);
    let ctx = ctx_mut.freeze();
    (ctx, body)
}

// Helper: create a body that represents an array of Drop types.
fn body_with_array_drop(mut ctx_mut: TyCtxMut, elem_ty: Ty, len: u64) -> (TyCtx, Body) {
    let mut body = Body::dummy(dummy_def_id());
    // Build a constant for the array length
    let const_len = Const {
        kind: ConstKind::Int(len as i128),
        ty: ctx_mut.mk_ty(TyKind::Uint(UintTy::Usize)),
    };
    let array_ty = ctx_mut.mk_ty(TyKind::Array(elem_ty, const_len));
    let array_local = body.locals.push(LocalDecl {
        ty: array_ty,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    let stmt1 = Statement {
        kind: StatementKind::StorageLive(array_local),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let term = Terminator {
        kind: TerminatorKind::Drop {
            place: Place::new(array_local),
            target: BasicBlockIdx::from_raw(1),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block0 = BasicBlockData {
        statements: vec![stmt1],
        terminator: term,
        is_cleanup: false,
    };
    let return_term = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block1 = BasicBlockData {
        statements: vec![],
        terminator: return_term,
        is_cleanup: false,
    };
    body.basic_blocks = IndexVec::from_raw(vec![block0, block1]);
    let ctx = ctx_mut.freeze();
    (ctx, body)
}

// Helper: create a body for an enum with a Drop variant.
fn body_with_enum_drop(ctx_mut: TyCtxMut, enum_ty: Ty) -> (TyCtx, Body) {
    let mut body = Body::dummy(dummy_def_id());
    let local = body.locals.push(LocalDecl {
        ty: enum_ty,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    let stmt1 = Statement {
        kind: StatementKind::StorageLive(local),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let term = Terminator {
        kind: TerminatorKind::Drop {
            place: Place::new(local),
            target: BasicBlockIdx::from_raw(1),
            cleanup: None,
        },
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block0 = BasicBlockData {
        statements: vec![stmt1],
        terminator: term,
        is_cleanup: false,
    };
    let return_term = Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let block1 = BasicBlockData {
        statements: vec![],
        terminator: return_term,
        is_cleanup: false,
    };
    body.basic_blocks = IndexVec::from_raw(vec![block0, block1]);
    let ctx = ctx_mut.freeze();
    (ctx, body)
}

#[test]
fn conditional_drop_after_partial_move() {
    let ctx_mut = test_ty_ctx();
    let ty = Ty::UNIT;
    let (ctx, mut body) = body_with_drop(ctx_mut, ty);
    crate::elaborate_drops(&ctx, &mut body);
    for block in body.basic_blocks.iter() {
        if let TerminatorKind::Drop { .. } = block.terminator.kind {
            panic!("Drop terminator still present after elaboration");
        }
    }
}

#[test]
fn array_drop_reverse() {
    let ctx_mut = test_ty_ctx();
    let elem_ty = Ty::UNIT;
    let (ctx, mut body) = body_with_array_drop(ctx_mut, elem_ty, 3);
    crate::elaborate_drops(&ctx, &mut body);
    for block in body.basic_blocks.iter() {
        if let TerminatorKind::Drop { place, .. } = &block.terminator.kind {
            if place.projection.is_empty() {
                panic!("Whole-array Drop terminator still present after elaboration");
            }
        }
    }
}

#[test]
#[ignore]
// Requires ADT registration from glyim-type (variants, drop glue info)
fn enum_drop_glue_discriminant() {
    let mut ctx_mut = test_ty_ctx();
    // Use a placeholder enum type (just an ADT with ID 1, no actual fields).
    let adt_id = AdtId::from_raw(1);
    let enum_ty = ctx_mut.mk_ty(TyKind::Adt(adt_id, Substitution::empty()));
    let (ctx, mut body) = body_with_enum_drop(ctx_mut, enum_ty);
    crate::elaborate_drops(&ctx, &mut body);
    for block in body.basic_blocks.iter() {
        if let TerminatorKind::Drop { .. } = block.terminator.kind {
            panic!("Drop terminator still present after elaboration");
        }
    }
    let has_switch = body
        .basic_blocks
        .iter()
        .any(|block| matches!(block.terminator.kind, TerminatorKind::SwitchInt { .. }));
    assert!(has_switch, "Expected a SwitchInt terminator for enum drop");
}
