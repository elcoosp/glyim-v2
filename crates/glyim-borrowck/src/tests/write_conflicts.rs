//! Tests for write conflicts — assigning to a place that is borrowed.

use crate::check_borrows;
use glyim_core::Mutability;
use glyim_mir::{BorrowKind, StatementKind};
use glyim_test::{assert_has_errors, assert_no_errors, with_fresh_ty_ctx};
use glyim_type::Region;

use super::mir_builder::{
    MirBodyBuilder, TestBorrowckCtx, assign_borrow, assign_copy, goto, ret,
};

/// Write to shared-borrowed place across blocks → error.
///
///   BB0: _2 = &shared _1; goto BB1
///   BB1: _1 = copy _3; _4 = copy _2; return
///
/// Even a shared borrow prevents writes to the borrowed place.
#[test]
fn write_to_shared_borrowed_place_across_blocks_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Mut);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(goto(1));
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Shared));

        let bb1 = b.push_block(ret());
        b.push_stmt(bb1, StatementKind::Assign(
            glyim_mir::Place::new(_1),
            glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(glyim_mir::Place::new(_3))),
        ));
        b.push_stmt(bb1, assign_copy(_4, _2));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_has_errors(&result.errors);
}

/// Write to shared-borrowed place after borrow expires → no error.
///
///   BB0: _2 = &shared _1; _3 = copy _2; goto BB1
///   BB1: _1 = copy _4; return
///
/// The borrow of _1 expires after _3 = copy _2 (last use of _2).
#[test]
fn write_after_shared_borrow_expires_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Mut);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(goto(1));
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Shared));
        b.push_stmt(bb0, assign_copy(_3, _2));  // last use of _2

        let bb1 = b.push_block(ret());
        b.push_stmt(bb1, StatementKind::Assign(
            glyim_mir::Place::new(_1),
            glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(glyim_mir::Place::new(_4))),
        ));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Write to mut-borrowed place across blocks → error.
///
///   BB0: _2 = &mut _1; goto BB1
///   BB1: _1 = copy _3; _4 = copy _2; return
#[test]
fn write_to_mut_borrowed_place_across_blocks_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_mut_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Mut);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Mut);
        let _2 = b.add_local(ref_mut_bool, Mutability::Mut);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(goto(1));
        b.push_stmt(
            bb0,
            assign_borrow(
                _2,
                _1,
                BorrowKind::Mut {
                    allow_two_phase_borrow: false,
                },
            ),
        );

        let bb1 = b.push_block(ret());
        b.push_stmt(bb1, StatementKind::Assign(
            glyim_mir::Place::new(_1),
            glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(glyim_mir::Place::new(_3))),
        ));
        b.push_stmt(bb1, assign_copy(_4, _2));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_has_errors(&result.errors);
}

/// Write to a different place while another is borrowed → no error.
///
///   BB0: _2 = &shared _1; goto BB1
///   BB1: _3 = copy _4; _5 = copy _2; return
#[test]
fn write_to_different_place_while_borrowed_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(bool_ty, Mutability::Mut);
        let _4 = b.add_local(bool_ty, Mutability::Not);
        let _5 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(goto(1));
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Shared));

        let bb1 = b.push_block(ret());
        b.push_stmt(bb1, StatementKind::Assign(
            glyim_mir::Place::new(_3),
            glyim_mir::Rvalue::Use(glyim_mir::Operand::Copy(glyim_mir::Place::new(_4))),
        ));
        b.push_stmt(bb1, assign_copy(_5, _2));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}
