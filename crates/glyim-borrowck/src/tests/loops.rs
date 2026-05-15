//! Tests for borrows in loop contexts.

use crate::check_borrows;
use glyim_core::Mutability;
use glyim_mir::BorrowKind;
use glyim_test::{assert_has_errors, assert_no_errors, with_fresh_ty_ctx};
use glyim_type::Region;

use super::mir_builder::{
    MirBodyBuilder, TestBorrowckCtx, assign_borrow, assign_copy, goto, if_switch, ret,
};

/// Shared borrow inside loop body, re-borrowed each iteration → no error.
///
///   BB0: goto BB1
///   BB1: switch _2 -> [true: BB2, false: BB3]
///   BB2: _4 = &shared _1; _5 = copy _4; goto BB1
///   BB3: return
#[test]
fn shared_borrow_inside_loop_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(ref_bool, Mutability::Not);
        let _5 = b.add_local(bool_ty, Mutability::Not);

        b.push_block(goto(1));

        b.push_block(if_switch(_2, bool_ty, 2, 3));

        let bb2 = b.push_block(goto(1));
        b.push_stmt(bb2, assign_borrow(_4, _1, BorrowKind::Shared));
        b.push_stmt(bb2, assign_copy(_5, _4));

        b.push_block(ret());

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Mutable borrow inside loop that conflicts with post-loop access → error.
///
///   BB0: goto BB1
///   BB1: switch _2 -> [true: BB2, false: BB3]
///   BB2: _4 = &mut _1; _5 = copy _4; goto BB1
///   BB3: _6 = copy _1; return
///
/// _4 is a fresh mut borrow each iteration, but it dies at the end of BB2.
/// After the loop, _1 is accessed directly. This should be OK because
/// the mut borrow's dest (_4) is not live after BB2.
#[test]
fn mut_borrow_inside_loop_then_access_after_ok() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_mut_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Mut);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(ref_mut_bool, Mutability::Mut);
        let _5 = b.add_local(bool_ty, Mutability::Not);
        let _6 = b.add_local(bool_ty, Mutability::Not);

        b.push_block(goto(1));

        b.push_block(if_switch(_2, bool_ty, 2, 3));

        let bb2 = b.push_block(goto(1));
        b.push_stmt(
            bb2,
            assign_borrow(
                _4,
                _1,
                BorrowKind::Mut {
                    allow_two_phase_borrow: false,
                },
            ),
        );
        b.push_stmt(bb2, assign_copy(_5, _4));

        let bb3 = b.push_block(ret());
        b.push_stmt(bb3, assign_copy(_6, _1));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Mut borrow before loop, accessed inside AND after loop → no error.
/// This tests that the borrow stays live across loop back-edges.
///
///   BB0: _2 = &mut _1; goto BB1
///   BB1: switch _3 -> [true: BB2, false: BB3]
///   BB2: _4 = copy _2; goto BB1
///   BB3: _5 = copy _2; return
#[test]
fn mut_borrow_before_loop_used_throughout_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_mut_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Mut);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_mut_bool, Mutability::Mut);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);
        let _5 = b.add_local(bool_ty, Mutability::Not);

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

        b.push_block(if_switch(_3, bool_ty, 2, 3));

        let bb2 = b.push_block(goto(1));
        b.push_stmt(bb2, assign_copy(_4, _2));

        let bb3 = b.push_block(ret());
        b.push_stmt(bb3, assign_copy(_5, _2));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Mut borrow before loop, conflicting access inside loop → error.
///
///   BB0: _2 = &mut _1; goto BB1
///   BB1: switch _3 -> [true: BB2, false: BB4]
///   BB2: _4 = copy _1; goto BB3    (direct read while mut borrowed!)
///   BB3: _5 = copy _2; goto BB1
///   BB4: _6 = copy _2; return
#[test]
fn mut_borrow_before_loop_conflicting_access_in_loop_error() {
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

        b.push_block(if_switch(_3, bool_ty, 2, 4));

        // BB2: conflicting direct read of _1
        let bb2 = b.push_block(goto(3));
        b.push_stmt(bb2, assign_copy(_4, _1));

        // BB3: use of _2 (keeps it live)
        let bb3 = b.push_block(goto(1));
        b.push_stmt(bb3, assign_copy(_5, _2));

        let bb4 = b.push_block(ret());
        b.push_stmt(bb4, assign_copy(_6, _2));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_has_errors(&result.errors);
}

/// Double loop (nested): shared borrow in outer, used in inner → no error.
///
///   BB0: _2 = &shared _1; goto BB1
///   BB1: switch _3 -> [true: BB2, false: BB5]   (outer loop header)
///   BB2: switch _4 -> [true: BB3, false: BB4]   (inner loop header)
///   BB3: _5 = copy _2; goto BB2                  (inner loop body)
///   BB4: goto BB1                                 (inner loop exit, back to outer)
///   BB5: return
#[test]
fn nested_loops_shared_borrow_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);
        let _5 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(goto(1));
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Shared));

        b.push_block(if_switch(_3, bool_ty, 2, 5));

        b.push_block(if_switch(_4, bool_ty, 3, 4));

        let bb3 = b.push_block(goto(2));
        b.push_stmt(bb3, assign_copy(_5, _2));

        b.push_block(goto(1));

        b.push_block(ret());

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}
