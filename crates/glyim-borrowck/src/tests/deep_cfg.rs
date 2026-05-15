//! Tests for deeply nested control flow with borrows.

use crate::check_borrows;
use glyim_core::Mutability;
use glyim_mir::BorrowKind;
use glyim_test::{assert_has_errors, assert_no_errors, with_fresh_ty_ctx};
use glyim_type::Region;

use super::mir_builder::{
    MirBodyBuilder, TestBorrowckCtx, assign_borrow, assign_copy, goto, if_switch, ret,
};

/// Nested if: shared borrow in outer scope, used in inner branches → no error.
///
/// MIR:
///   BB0: _2 = &shared _1; goto BB1
///   BB1: switch _3 -> [true: BB2, false: BB5]
///   BB2: switch _4 -> [true: BB3, false: BB4]
///   BB3: _5 = copy _2; goto BB6
///   BB4: _6 = copy _2; goto BB6
///   BB5: _7 = copy _2; goto BB6
///   BB6: return
#[test]
fn nested_if_shared_borrow_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);
        let _5 = b.add_local(bool_ty, Mutability::Not);
        let _6 = b.add_local(bool_ty, Mutability::Not);
        let _7 = b.add_local(bool_ty, Mutability::Not);

        // BB0
        let bb0 = b.push_block(goto(1));
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Shared));

        // BB1: outer switch
        b.push_block(if_switch(_3, bool_ty, 2, 5));

        // BB2: inner switch
        b.push_block(if_switch(_4, bool_ty, 3, 4));

        // BB3: inner then
        let bb3 = b.push_block(goto(6));
        b.push_stmt(bb3, assign_copy(_5, _2));

        // BB4: inner else
        let bb4 = b.push_block(goto(6));
        b.push_stmt(bb4, assign_copy(_6, _2));

        // BB5: outer else
        let bb5 = b.push_block(goto(6));
        b.push_stmt(bb5, assign_copy(_7, _2));

        // BB6
        b.push_block(ret());

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Nested if: mutable borrow in outer scope, conflicting access in inner branch → error.
///
/// MIR:
///   BB0: _2 = &mut _1; goto BB1
///   BB1: switch _3 -> [true: BB2, false: BB4]
///   BB2: switch _4 -> [true: BB3, false: BB5]
///   BB3: _5 = copy _2; goto BB6          (use mut ref, OK)
///   BB4: _6 = copy _1; goto BB6          (direct read while mut borrowed, ERROR)
///   BB5: _7 = copy _2; goto BB6          (use mut ref, OK)
///   BB6: return
#[test]
fn nested_if_mut_borrow_conflict_in_branch_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_mut_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Mut);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_mut_bool, Mutability::Mut);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);
        let _5 = b.add_local(bool_ty, Mutability::Not);
        let _6 = b.add_local(bool_ty, Mutability::Not);
        let _7 = b.add_local(bool_ty, Mutability::Not);

        // BB0
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

        // BB1: outer switch
        b.push_block(if_switch(_3, bool_ty, 2, 4));

        // BB2: inner switch
        b.push_block(if_switch(_4, bool_ty, 3, 5));

        // BB3: inner then — use mut ref
        let bb3 = b.push_block(goto(6));
        b.push_stmt(bb3, assign_copy(_5, _2));

        // BB4: outer else — direct read of _1 while mut borrowed
        let bb4 = b.push_block(goto(6));
        b.push_stmt(bb4, assign_copy(_6, _1));

        // BB5: inner else — use mut ref
        let bb5 = b.push_block(goto(6));
        b.push_stmt(bb5, assign_copy(_7, _2));

        // BB6
        b.push_block(ret());

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_has_errors(&result.errors);
}

/// Diamond CFG: shared borrow created before branch, used after merge → no error.
///
///   BB0: _2 = &shared _1; goto BB1
///   BB1: switch _3 -> [true: BB2, false: BB3]
///   BB2: goto BB4
///   BB3: goto BB4
///   BB4: _4 = copy _2; return
#[test]
fn diamond_cfg_shared_borrow_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(goto(1));
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Shared));

        b.push_block(if_switch(_3, bool_ty, 2, 3));

        b.push_block(goto(4));
        b.push_block(goto(4));

        let bb4 = b.push_block(ret());
        b.push_stmt(bb4, assign_copy(_4, _2));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Shared borrow in one branch of diamond, used after merge.
/// The borrow is only created on one path but the ref local is live on both.
///
///   BB0: switch _3 -> [true: BB1, false: BB2]
///   BB1: _2 = &shared _1; goto BB3
///   BB2: goto BB3
///   BB3: _4 = copy _2; return
///
/// This should error because _2 may be uninitialized on the BB2 path,
/// but for borrowck purposes we check what happens if the borrow IS live.
/// The borrow of _1 only exists on BB1 path, and _4 = copy _2 on BB3
/// means _2 is live-out of BB1. Since _2 is not defined on BB2, this
/// is actually an uninitialized-local issue, not a borrow issue.
/// Borrowck should not flag a borrow conflict here.
#[test]
fn borrow_in_one_branch_of_diamond_no_conflict() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);

        b.push_block(if_switch(_3, bool_ty, 1, 2));

        let bb1 = b.push_block(goto(3));
        b.push_stmt(bb1, assign_borrow(_2, _1, BorrowKind::Shared));

        b.push_block(goto(3));

        let bb3 = b.push_block(ret());
        b.push_stmt(bb3, assign_copy(_4, _2));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    // No borrow conflict — the borrow only exists on one path
    assert_no_errors(&result.errors);
}
