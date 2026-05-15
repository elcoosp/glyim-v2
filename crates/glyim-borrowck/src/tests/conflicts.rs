//! Tests for conflicting borrows across blocks (V08-T05, T06, T07).

use crate::check_borrows;
use glyim_core::Mutability;
use glyim_mir::BorrowKind;
use glyim_test::{assert_has_errors, assert_no_errors, with_fresh_ty_ctx};
use glyim_type::Region;

use super::mir_builder::{MirBodyBuilder, TestBorrowckCtx, assign_borrow, assign_copy, goto, if_switch, ret};

/// V08-T05: Conflicting loans across blocks → error.
///
/// MIR:
///   BB0: _2 = &mut _1; goto BB1
///   BB1: _3 = &shared _1; goto BB2
///   BB2: _4 = copy _2; return
#[test]
fn conflicting_loans_across_blocks_error() {
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
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Mut { allow_two_phase_borrow: false }));

        let bb1 = b.push_block(goto(2));
        b.push_stmt(bb1, assign_borrow(_3, _1, BorrowKind::Shared));

        let bb2 = b.push_block(ret());
        b.push_stmt(bb2, assign_copy(_4, _2));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_has_errors(&result.errors);
}

/// V08-T06: Direct access to mutably borrowed place across blocks → error.
///
/// MIR:
///   BB0: _2 = &mut _1; goto BB1
///   BB1: _3 = copy _1; _4 = copy _2; return
#[test]
fn direct_access_while_mutably_borrowed_across_blocks_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_mut_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Mut);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_mut_bool, Mutability::Mut);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);

        let bb0 = b.push_block(goto(1));
        b.push_stmt(bb0, assign_borrow(_2, _1, BorrowKind::Mut { allow_two_phase_borrow: false }));

        let bb1 = b.push_block(ret());
        b.push_stmt(bb1, assign_copy(_3, _1));
        b.push_stmt(bb1, assign_copy(_4, _2));

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_has_errors(&result.errors);
}

/// V08-T07: Path-sensitive — borrows on different branches → no error.
///
/// MIR:
///   BB0: switch _3 -> [true: BB1, false: BB2]
///   BB1: _2 = &mut _1; _4 = copy _2; goto BB3
///   BB2: _5 = &shared _1; _6 = copy _5; goto BB3
///   BB3: return
#[test]
fn path_sensitive_borrows_on_different_branches_no_error() {
    let (ty_ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let bool_ty = ctx_mut.bool_ty();
        let ref_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Not);
        let ref_mut_bool = ctx_mut.mk_ref(Region::Erased, bool_ty, Mutability::Mut);

        let mut b = MirBodyBuilder::new(bool_ty);
        let _1 = b.add_local(bool_ty, Mutability::Not);
        let _2 = b.add_local(ref_mut_bool, Mutability::Mut);
        let _3 = b.add_local(bool_ty, Mutability::Not);
        let _4 = b.add_local(bool_ty, Mutability::Not);
        let _5 = b.add_local(ref_bool, Mutability::Not);
        let _6 = b.add_local(bool_ty, Mutability::Not);

        let _bb0 = b.push_block(if_switch(_3, bool_ty, 1, 2));

        let bb1 = b.push_block(goto(3));
        b.push_stmt(bb1, assign_borrow(_2, _1, BorrowKind::Mut { allow_two_phase_borrow: false }));
        b.push_stmt(bb1, assign_copy(_4, _2));

        let bb2 = b.push_block(goto(3));
        b.push_stmt(bb2, assign_borrow(_5, _1, BorrowKind::Shared));
        b.push_stmt(bb2, assign_copy(_6, _5));

        b.push_block(ret());

        b.build()
    });

    let mock_ctx = TestBorrowckCtx::new(&ty_ctx, &body);
    let result = check_borrows(&mock_ctx, &body);
    assert_no_errors(&result.errors);
}
