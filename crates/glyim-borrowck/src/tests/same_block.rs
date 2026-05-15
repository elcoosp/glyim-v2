//! Tests verifying that the original single-block borrow checking still works
//! correctly after the Polonius refactoring.

use crate::check_borrows;
use glyim_core::Mutability;
use glyim_mir::BorrowKind;
use glyim_test::{assert_has_errors, assert_no_errors, with_fresh_ty_ctx};
use glyim_type::Region;

use super::mir_builder::{MirBodyBuilder, TestBorrowckCtx, assign_borrow, assign_copy, ret};

/// Shared borrow followed by shared borrow in same block → no error.
#[test]
fn same_block_shared_shared_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(ref_bool, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);
        let _5 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(ret());
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Shared));
        b.push_stmt(bb0, assign_borrow(_3, _1, BorrowKind::Shared));
        b.push_stmt(bb0, assign_copy(_4, _2));
        b.push_stmt(bb0, assign_copy(_5, _3));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Mutable borrow followed by shared borrow in same block → error.
#[test]
fn same_block_mut_shared_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);
        let ref_mut_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Mut);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_mut_bool, Mutability::Mut);
        let _3 = b.add_local(ref_bool, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);
        let _5 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(ret());
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
        b.push_stmt(bb0, assign_borrow(_3, _1, BorrowKind::Shared));
        b.push_stmt(bb0, assign_copy(_4, _2));
        b.push_stmt(bb0, assign_copy(_5, _3));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_has_errors(&result.errors);
}

/// Shared borrow expires (last use before second borrow) in same block → no error.
#[test]
fn same_block_borrow_expires_before_conflict_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);
        let ref_mut_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Mut);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(ref_mut_bool, Mutability::Mut);
        let _5 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(ret());
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Shared));
        b.push_stmt(bb0, assign_copy(_3, _2)); // last use of _2
        b.push_stmt(
            bb0,
            assign_borrow(
                _4,
                _1,
                BorrowKind::Mut {
                    allow_two_phase_borrow: false,
                },
            ),
        );
        b.push_stmt(bb0, assign_copy(_5, _4));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// Two mutable borrows of the same place in same block → error.
#[test]
fn same_block_two_mut_borrows_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_mut_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Mut);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_mut_bool, Mutability::Mut);
        let _3 = b.add_local(ref_mut_bool, Mutability::Mut);
        let _4 = b.add_local(bool_ty, Mutability::Not);
        let _5 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(ret());
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
        b.push_stmt(
            bb0,
            assign_borrow(
                _3,
                _1,
                BorrowKind::Mut {
                    allow_two_phase_borrow: false,
                },
            ),
        );
        b.push_stmt(bb0, assign_copy(_4, _2));
        b.push_stmt(bb0, assign_copy(_5, _3));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_has_errors(&result.errors);
}
