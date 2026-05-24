//! Tests for drop elaboration: conditional drops, array loops, enum discriminants.

use glyim_core::primitives::{BinOp, Mutability, UnOp};
use glyim_core::{AdtId, DefId, IndexVec};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{Const, Substitution, Ty, TyCtx, TyCtxMut, TyKind};
use glyim_test::{assert_mir, test_frozen_ty_ctx, test_ty_ctx};

// Helper: create a simple MIR body with a Drop terminator for a single local.
// Uses TyCtxMut for construction, then freezes for assertion.
fn body_with_drop(ctx_mut: &mut TyCtxMut, local_ty: Ty) -> (TyCtx, Body) {
    let mut body = Body::dummy(DefId::from_raw(0, 0));
    let local = body.locals.push(LocalDecl {
        ty: local_ty,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    // StorageLive(local)
    let stmt1 = Statement {
        kind: StatementKind::StorageLive(local),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    // Drop terminator
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
    // Return block
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

// Helper: create a body that represents an array of Drop types (simulated with a loop)
fn body_with_array_drop(ctx_mut: &mut TyCtxMut, elem_ty: Ty, len: u64) -> (TyCtx, Body) {
    let mut body = Body::dummy(DefId::from_raw(0, 0));
    let array_ty = ctx_mut.mk_ty(TyKind::Array(elem_ty, Const::from_usize(ctx_mut, len)));
    let array_local = body.locals.push(LocalDecl {
        ty: array_ty,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    // StorageLive(array)
    let stmt1 = Statement {
        kind: StatementKind::StorageLive(array_local),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    // Simulate a loop that iterates over indices and drops each element.
    // For test, we just put a Drop terminator for the whole array (should be expanded).
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
fn body_with_enum_drop(ctx_mut: &mut TyCtxMut, enum_ty: Ty) -> (TyCtx, Body) {
    let mut body = Body::dummy(DefId::from_raw(0, 0));
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
    let mut ctx_mut = test_ty_ctx();
    // Use a type that is known to be Drop (e.g., a dummy struct with Drop impl).
    // For testing, we can use a simple type that we mark as needing drop.
    let ty = ctx_mut.mk_ty(TyKind::Adt(AdtId::from_raw(0), Substitution::empty()));
    let (ctx, mut body) = body_with_drop(&mut ctx_mut, ty);
    // Run drop elaboration (initially a no-op stub)
    crate::elaborate_drops(&ctx, &mut body);
    // After elaboration, the Drop terminator should have been replaced.
    // We check that no Drop terminator remains in any block.
    for block in body.basic_blocks.iter() {
        if let TerminatorKind::Drop { .. } = block.terminator.kind {
            panic!("Drop terminator still present after elaboration");
        }
    }
    // Additionally, we can check that the body now contains a conditional branch based on drop flag.
    // This will be implemented later.
}

#[test]
fn array_drop_reverse() {
    let mut ctx_mut = test_ty_ctx();
    let elem_ty = ctx_mut.mk_ty(TyKind::Adt(AdtId::from_raw(0), Substitution::empty()));
    let (ctx, mut body) = body_with_array_drop(&mut ctx_mut, elem_ty, 3);
    crate::elaborate_drops(&ctx, &mut body);
    // After elaboration, the Drop terminator for the whole array should be replaced
    // by a loop that drops each element in reverse order.
    // We check that there are no Drop terminators on the array itself.
    for block in body.basic_blocks.iter() {
        if let TerminatorKind::Drop { place, .. } = &block.terminator.kind {
            if place.projection.is_empty() {
                panic!("Whole-array Drop terminator still present after elaboration");
            }
        }
    }
    // TODO: More detailed check for the loop structure.
}

#[test]
fn enum_drop_glue_discriminant() {
    let mut ctx_mut = test_ty_ctx();
    // Simulate an enum type (we don't need full def, just a placeholder).
    let enum_ty = ctx_mut.mk_ty(TyKind::Adt(AdtId::from_raw(1), Substitution::empty()));
    let (ctx, mut body) = body_with_enum_drop(&mut ctx_mut, enum_ty);
    crate::elaborate_drops(&ctx, &mut body);
    // After elaboration, the Drop terminator should be replaced by a switch on the
    // discriminant, dropping only variants that contain drop types.
    for block in body.basic_blocks.iter() {
        if let TerminatorKind::Drop { .. } = block.terminator.kind {
            panic!("Drop terminator still present after elaboration");
        }
    }
    // Check that a SwitchInt terminator appears.
    let has_switch = body.basic_blocks.iter().any(|block| {
        matches!(block.terminator.kind, TerminatorKind::SwitchInt { .. })
    });
    assert!(has_switch, "Expected a SwitchInt terminator for enum drop");
}
