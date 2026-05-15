//! Tests for multiple loans with subset constraints (V08-T04).

use crate::check_borrows;
use glyim_core::Mutability;
use glyim_mir::BorrowKind;
use glyim_test::{assert_no_errors, with_fresh_ty_ctx};
use glyim_type::Region;

use super::mir_builder::{MirBodyBuilder, TestBorrowckCtx, assign_borrow, assign_copy, goto, ret};

/// V08-T04: Multiple loans, subset constraints → no error.
///
/// Two shared borrows of the same place. Shared borrows are compatible
/// with each other, so having multiple shared borrows active at the same
/// time should not produce an error.
///
/// MIR:
///   BB0: _2 = &shared _1; _3 = &shared _1; goto BB1
///   BB1: _4 = copy _2; _5 = copy _3; return
#[test]
fn multiple_shared_loans_no_error() {
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
        b.push_stmt(bb0, assign_borrow(_3, _1, BorrowKind::Shared));

        let bb1 = b.push_block(ret());
        b.push_stmt(bb1, assign_copy(_4, _2));
        b.push_stmt(bb1, assign_copy(_5, _3));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}

/// V08-T04b: Nested borrows — shared ref to a shared ref → no error.
///
/// MIR:
///   BB0: _2 = &shared _1; _3 = &shared _2; goto BB1
///   BB1: _4 = copy _3; return
#[test]
fn nested_shared_borrows_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);
        let ref_ref_bool = ctx_mut.mk_ref(Region::Erased, ref_bool, Mutability::Not);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_bool, Mutability::Not);
        let _3 = b.add_local(ref_ref_bool, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(goto(1));
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Shared));
        b.push_stmt(bb0, assign_borrow(_3, _2, BorrowKind::Shared));

        let bb1 = b.push_block(ret());
        b.push_stmt(bb1, assign_copy(_4, _3));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}
