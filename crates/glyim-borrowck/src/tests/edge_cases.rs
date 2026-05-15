//! Edge case tests for the borrow checker.

use crate::check_borrows;
use glyim_core::Mutability;
use glyim_mir::{BorrowKind, StatementKind};
use glyim_test::{assert_has_errors, assert_no_errors, with_fresh_ty_ctx};
use glyim_type::Region;

use super::mir_builder::{MirBodyBuilder, TestBorrowckCtx, assign_borrow, assign_copy, goto, ret};

/// Body with no borrows at all → no error.
#[test]
fn no_borrows_at_all_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(ret());
        b.push_stmt(bb0, assign_copy(_2, _1));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Body with a single basic block and a self-contained borrow → no error.
#[test]
fn single_block_borrow_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(ret());
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Shared));
        b.push_stmt(bb0, assign_copy(_3, _2));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Borrow that is never used — dest local is dead → no conflict even with
/// a second borrow of the same place.
///
///   BB0: _2 = &mut _1; goto BB1    (_2 is never read)
///   BB1: _3 = &shared _1; _4 = copy _3; return
#[test]
fn dead_borrow_no_conflict() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);
        let ref_mut_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Mut);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_mut_bool, Mutability::Mut);
        let _3 = b.add_local(ref_bool, Mutability::Not);
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
        b.push_stmt(bb1, assign_borrow(_3, _1, BorrowKind::Shared));
        b.push_stmt(bb1, assign_copy(_4, _3));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Body with only a return — no statements at all.
#[test]
fn empty_body_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let mut b = MirBodyBuilder::new(bool_ty);
        b.push_block(ret());
        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Borrow of different places — should not conflict.
///
///   BB0: _2 = &mut _1; _4 = &shared _3; goto BB1
///   BB1: _5 = copy _2; _6 = copy _4; return
#[test]
fn borrows_of_different_places_no_conflict() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);
        let ref_mut_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Mut);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_mut_bool, Mutability::Mut);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(ref_bool, Mutability::Not);
        let _5 = b.add_local(bool_ty, Mutability::Not);
        let _6 = b.add_local(bool_ty, Mutability::Not);

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
        b.push_stmt(bb0, assign_borrow(_4, _3, BorrowKind::Shared));

        let bb1 = b.push_block(ret());
        b.push_stmt(bb1, assign_copy(_5, _2));
        b.push_stmt(bb1, assign_copy(_6, _4));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Multiple shared borrows of the same place across different blocks → no error.
///
///   BB0: _2 = &shared _1; goto BB1
///   BB1: _3 = &shared _1; goto BB2
///   BB2: _4 = copy _2; _5 = copy _3; return
#[test]
fn multiple_shared_borrows_across_blocks_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(ref_bool, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);
        let _5 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(goto(1));
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Shared));

        let bb1 = b.push_block(goto(2));
        b.push_stmt(bb1, assign_borrow(_3, _1, BorrowKind::Shared));

        let bb2 = b.push_block(ret());
        b.push_stmt(bb2, assign_copy(_4, _2));
        b.push_stmt(bb2, assign_copy(_5, _3));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Body with StorageLive/StorageDead statements — should be ignored.
///
///   BB0: StorageLive(_1); _2 = &shared _1; _3 = copy _2; StorageDead(_1); return
#[test]
fn storage_live_dead_ignored_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(ret());
        b.push_stmt(bb0, StatementKind::StorageLive(_1));
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Shared));
        b.push_stmt(bb0, assign_copy(_3, _2));
        b.push_stmt(bb0, StatementKind::StorageDead(_1));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Unique borrow conflicts with shared borrow on same place → error.
///
///   BB0: _2 = &unique _1; goto BB1
///   BB1: _3 = &shared _1; _4 = copy _2; return
#[test]
fn unique_and_shared_conflict_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);
        let ref_unique_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_unique_bool, Mutability::Not);
        let _3 = b.add_local(ref_bool, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(goto(1));
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Unique));

        let bb1 = b.push_block(ret());
        b.push_stmt(bb1, assign_borrow(_3, _1, BorrowKind::Shared));
        b.push_stmt(bb1, assign_copy(_4, _2));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_has_errors(&result.errors);
}

/// Unique borrow expires, then shared borrow of same place → no error.
///
///   BB0: _2 = &unique _1; _3 = copy _2; goto BB1   (last use of _2)
///   BB1: _4 = &shared _1; _5 = copy _4; return
#[test]
fn unique_expires_then_shared_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);
        let ref_unique_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_unique_bool, Mutability::Not);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(ref_bool, Mutability::Not);
        let _5 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(goto(1));
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Unique));
        b.push_stmt(bb0, assign_copy(_3, _2));

        let bb1 = b.push_block(ret());
        b.push_stmt(bb1, assign_borrow(_4, _1, BorrowKind::Shared));
        b.push_stmt(bb1, assign_copy(_5, _4));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}
