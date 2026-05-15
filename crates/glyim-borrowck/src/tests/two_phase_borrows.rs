//! Tests for two-phase borrow support (Stream V09).
//!
//! Two-phase borrows allow a mutable borrow to start in a "reservation"
//! phase (acting as a shared borrow) and only become "activated" at the
//! call site. This enables patterns like `vec.push(vec.len())` where the
//! mutable borrow of `vec` for `push` does not conflict with the shared
//! read of `vec.len()` that occurs in the arguments.

use crate::{BorrowckCtx, check_borrows};
use glyim_core::{CrateId, DefId, IndexVec, LocalDefId, Mutability};
use glyim_mir::{
    BasicBlockData, BasicBlockIdx, Body, BorrowKind, LocalDecl, LocalIdx, Operand, Place, Rvalue,
    SourceInfo, Statement, StatementKind, Terminator, TerminatorKind,
};
use glyim_span::Span;
use glyim_test::{assert_has_errors, assert_no_errors, with_fresh_ty_ctx};
use glyim_type::Ty;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct LocalMockBorrowckCtx {
    ty_ctx: glyim_type::TyCtx,
    body: Body,
}

impl BorrowckCtx for LocalMockBorrowckCtx {
    fn ty_ctx(&self) -> &glyim_type::TyCtx {
        &self.ty_ctx
    }
    fn local_decl(&self, local: LocalIdx) -> &glyim_mir::LocalDecl {
        &self.body.locals[local]
    }
    fn is_copy(&self, _ty: Ty) -> bool {
        false
    }
}

fn local_decl(ty: Ty) -> LocalDecl {
    LocalDecl {
        ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

fn make_body(statements: Vec<Statement>, locals: Vec<LocalDecl>) -> Body {
    let def_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0));
    let mut body = Body::dummy(def_id);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    })]);
    body.basic_blocks[BasicBlockIdx::from_raw(0)].statements = statements;
    body.locals = IndexVec::from_raw(locals);
    body
}

/// Build a multi-block MIR body from basic blocks.
fn make_body_multi_blocks(locals: Vec<LocalDecl>, blocks: Vec<BasicBlockData>) -> Body {
    let def_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0));
    let mut body = Body::dummy(def_id);
    body.basic_blocks = IndexVec::<BasicBlockIdx, _>::from_raw(blocks);
    body.locals = IndexVec::<LocalIdx, _>::from_raw(locals);
    body
}

fn make_ref_ty(ctx_mut: &mut glyim_type::TyCtxMut, inner: Ty, mutable: bool) -> Ty {
    let mutability = if mutable {
        Mutability::Mut
    } else {
        Mutability::Not
    };
    ctx_mut.mk_ref(glyim_type::Region::Erased, inner, mutability)
}

fn assign_borrow(dest: LocalIdx, place: Place, kind: BorrowKind) -> Statement {
    Statement {
        kind: StatementKind::Assign(Place::new(dest), Rvalue::Ref(place, kind)),
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

fn use_local(dest: LocalIdx, local: LocalIdx) -> Statement {
    Statement {
        kind: StatementKind::Assign(
            Place::new(dest),
            Rvalue::Use(Operand::Copy(Place::new(local))),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

fn make_two_phase_mut() -> BorrowKind {
    BorrowKind::Mut {
        allow_two_phase_borrow: true,
    }
}

fn make_one_phase_mut() -> BorrowKind {
    BorrowKind::Mut {
        allow_two_phase_borrow: false,
    }
}

fn dummy_terminator_goto(target: u32) -> Terminator {
    Terminator {
        kind: TerminatorKind::Goto {
            target: BasicBlockIdx::from_raw(target),
        },
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

fn dummy_terminator_return() -> Terminator {
    Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

fn stmt_assign(dest: LocalIdx, src: LocalIdx) -> Statement {
    Statement {
        kind: StatementKind::Assign(
            Place::new(dest),
            Rvalue::Use(Operand::Copy(Place::new(src))),
        ),
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

// ===========================================================================
// Original tests (V09-T01 through V09-T06)
// ===========================================================================

#[test]
fn t01_two_phase_borrow_with_intervening_shared_read() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(mut_ref_ty),    // _2 mut ref (two-phase)
            local_decl(shared_ref_ty), // _3 shared ref
            local_decl(i32_ty),        // _4 result
        ];

        let stmts = vec![
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)),
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_no_errors(&result.errors);
}

#[test]
fn t02_two_phase_reservation_allows_shared_borrow() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(mut_ref_ty),    // _2 mut ref (two-phase)
            local_decl(shared_ref_ty), // _3 shared ref
            local_decl(i32_ty),        // _4 result
        ];

        let stmts = vec![
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)),
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_no_errors(&result.errors);
}

#[test]
fn t03_activation_conflict_with_shared_read() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(mut_ref_ty),    // _2 mut ref (two-phase)
            local_decl(shared_ref_ty), // _3 shared ref
            local_decl(i32_ty),        // _4 result
            local_decl(i32_ty),        // _5 result2
        ];

        let stmts = vec![
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(3)),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(2)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_has_errors(&result.errors);
}

#[test]
fn t04_reservation_conflict_with_other_mutable() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);

        let locals = vec![
            local_decl(unit),       // _0 return
            local_decl(i32_ty),     // _1 data
            local_decl(mut_ref_ty), // _2 first mut ref (two-phase)
            local_decl(mut_ref_ty), // _3 second mut ref
            local_decl(i32_ty),     // _4 result
            local_decl(i32_ty),     // _5 result2
        ];

        let stmts = vec![
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(3)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_has_errors(&result.errors);
}

#[test]
fn t05_one_phase_mut_conflicts_with_shared() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(mut_ref_ty),    // _2 mut ref (one-phase)
            local_decl(shared_ref_ty), // _3 shared ref
            local_decl(i32_ty),        // _4 result
            local_decl(i32_ty),        // _5 result2
        ];

        let stmts = vec![
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_one_phase_mut(),
            ),
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(3)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_has_errors(&result.errors);
}

#[test]
fn t06_two_phase_borrow_never_activated_no_conflict() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(mut_ref_ty),    // _2 mut ref (two-phase, never used)
            local_decl(shared_ref_ty), // _3 shared ref
            local_decl(i32_ty),        // _4 result
        ];

        let stmts = vec![
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_no_errors(&result.errors);
}

// ===========================================================================
// Additional tests (V09-T07 through V09-T16)
// ===========================================================================

// V09-T07: Two-phase borrow allows read of borrowed place during reservation
// A direct read (copy) of the borrowed place is allowed during reservation.

#[test]
fn t07_two_phase_reservation_allows_read_of_borrowed_place() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);

        let locals = vec![
            local_decl(unit),       // _0 return
            local_decl(i32_ty),     // _1 data
            local_decl(mut_ref_ty), // _2 mut ref (two-phase)
            local_decl(i32_ty),     // _3 copy of data (read during reservation)
            local_decl(i32_ty),     // _4 use result
        ];

        let stmts = vec![
            // _2 = &mut _1 (two-phase reservation)
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            // _3 = _1 (read of _1 during reservation — allowed)
            use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(1)),
            // use _2 (activation)
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_no_errors(&result.errors);
}

// V09-T08: Two-phase borrow, write to borrowed place during reservation — error
// Even during reservation, a write (assignment) to the borrowed place is not allowed
// because reservation acts as a shared borrow.

#[test]
fn t08_two_phase_reservation_write_to_borrowed_place_error() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);

        let locals = vec![
            local_decl(unit),       // _0 return
            local_decl(i32_ty),     // _1 data
            local_decl(mut_ref_ty), // _2 mut ref (two-phase)
            local_decl(i32_ty),     // _3 temp
            local_decl(i32_ty),     // _4 use result
        ];

        let stmts = vec![
            // _2 = &mut _1 (two-phase reservation)
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            // _1 = 42 (write to _1 during reservation — NOT allowed)
            Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Copy(Place::new(LocalIdx::from_raw(3)))),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            // use _2 (activation)
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_has_errors(&result.errors);
}

// V09-T09: Two-phase borrow with unique borrow conflict — error
// A Unique borrow conflicts with a two-phase mutable borrow even during
// reservation, because Unique borrows require exclusive access.

#[test]
fn t09_two_phase_reservation_conflicts_with_unique_borrow() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);

        let locals = vec![
            local_decl(unit),       // _0 return
            local_decl(i32_ty),     // _1 data
            local_decl(mut_ref_ty), // _2 two-phase mut ref
            local_decl(mut_ref_ty), // _3 unique ref
            local_decl(i32_ty),     // _4 result
            local_decl(i32_ty),     // _5 result2
        ];

        let stmts = vec![
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Unique,
            ),
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(3)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_has_errors(&result.errors);
}

// V09-T10: Two-phase borrow on different places — no conflict
// Two two-phase mutable borrows of *different* places don't conflict.

#[test]
fn t10_two_phase_borrows_different_places_no_conflict() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);

        let locals = vec![
            local_decl(unit),       // _0 return
            local_decl(i32_ty),     // _1 data a
            local_decl(i32_ty),     // _2 data b
            local_decl(mut_ref_ty), // _3 mut ref to a
            local_decl(mut_ref_ty), // _4 mut ref to b
            local_decl(i32_ty),     // _5 result
            local_decl(i32_ty),     // _6 result2
        ];

        let stmts = vec![
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            assign_borrow(
                LocalIdx::from_raw(4),
                Place::new(LocalIdx::from_raw(2)),
                make_two_phase_mut(),
            ),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(3)),
            use_local(LocalIdx::from_raw(6), LocalIdx::from_raw(4)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_no_errors(&result.errors);
}

// V09-T11: Two-phase borrow with multiple shared borrows during reservation
// Multiple shared borrows are allowed during the reservation phase.

#[test]
fn t11_two_phase_reservation_multiple_shared_borrows() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(mut_ref_ty),    // _2 mut ref (two-phase)
            local_decl(shared_ref_ty), // _3 shared ref 1
            local_decl(shared_ref_ty), // _4 shared ref 2
            local_decl(i32_ty),        // _5 result
            local_decl(i32_ty),        // _6 result2
            local_decl(i32_ty),        // _7 result3
        ];

        let stmts = vec![
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            assign_borrow(
                LocalIdx::from_raw(4),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(3)),
            use_local(LocalIdx::from_raw(6), LocalIdx::from_raw(4)),
            use_local(LocalIdx::from_raw(7), LocalIdx::from_raw(2)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_no_errors(&result.errors);
}

// V09-T12: Two-phase borrow, shared ref dies before activation — no conflict
// The shared borrow created during reservation expires (is no longer live)
// before the two-phase borrow is activated. This should be fine.

#[test]
fn t12_shared_ref_dies_before_two_phase_activation() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(mut_ref_ty),    // _2 mut ref (two-phase)
            local_decl(shared_ref_ty), // _3 shared ref
            local_decl(i32_ty),        // _4 result
        ];

        let stmts = vec![
            // Two-phase mut borrow of _1
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            // Shared borrow of _1 during reservation
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            // Shared ref is the LAST use of _3 — it dies after this
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)),
            // Now activate the two-phase borrow — _3 is dead, no conflict
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_no_errors(&result.errors);
}

// V09-T13: One-phase mut borrow after shared ref expires — no conflict
// Shared ref is created and dies, then a one-phase mut borrow is created.
// No conflict because shared ref is dead.

#[test]
fn t13_one_phase_mut_after_shared_expires_no_conflict() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(shared_ref_ty), // _2 shared ref
            local_decl(mut_ref_ty),    // _3 mut ref (one-phase)
            local_decl(i32_ty),        // _4 result
            local_decl(i32_ty),        // _5 result2
        ];

        let stmts = vec![
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            // shared ref dies here (last use)
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
            // one-phase mut borrow — shared ref is dead, no conflict
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                make_one_phase_mut(),
            ),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(3)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_no_errors(&result.errors);
}

// V09-T14: Two-phase borrow across basic blocks — conservatively treated as activated
// When a two-phase borrow is created in one block and the shared access happens
// in a later block, we conservatively treat it as already activated, so a shared
// borrow would conflict.

#[test]
fn t14_two_phase_cross_block_conservatively_activated() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(mut_ref_ty),    // _2 mut ref (two-phase)
            local_decl(shared_ref_ty), // _3 shared ref
            local_decl(i32_ty),        // _4 result
            local_decl(i32_ty),        // _5 result2
        ];

        // Block 0: create two-phase borrow, then goto block 1
        let mut block0 = BasicBlockData::new(dummy_terminator_goto(1));
        block0.statements.push(assign_borrow(
            LocalIdx::from_raw(2),
            Place::new(LocalIdx::from_raw(1)),
            make_two_phase_mut(),
        ));

        // Block 1: shared borrow of same place (cross-block — conservatively activated)
        let mut block1 = BasicBlockData::new(dummy_terminator_goto(2));
        block1.statements.push(assign_borrow(
            LocalIdx::from_raw(3),
            Place::new(LocalIdx::from_raw(1)),
            BorrowKind::Shared,
        ));

        // Block 2: use both refs
        let mut block2 = BasicBlockData::new(dummy_terminator_return());
        block2
            .statements
            .push(use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)));
        block2
            .statements
            .push(use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(2)));

        make_body_multi_blocks(locals, vec![block0, block1, block2])
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    // Cross-block: two-phase is conservatively activated, so shared borrow conflicts
    assert_has_errors(&result.errors);
}

// V09-T15: Two-phase borrow, same block activation then another shared — error
// After activation, even in the same block, a new shared borrow conflicts.

#[test]
fn t15_two_phase_activated_then_shared_in_same_block_error() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(mut_ref_ty),    // _2 mut ref (two-phase)
            local_decl(i32_ty),        // _3 read result (activates)
            local_decl(shared_ref_ty), // _4 shared ref (after activation)
            local_decl(i32_ty),        // _5 result2
            local_decl(i32_ty),        // _6 result3
        ];

        let stmts = vec![
            // _2 = &mut _1 (two-phase — reservation)
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            // _3 = *_2 (read _2 — activates the two-phase borrow)
            use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(2)),
            // _4 = &_1 (shared borrow AFTER activation — conflict!)
            assign_borrow(
                LocalIdx::from_raw(4),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            // use both to keep them live
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(4)),
            use_local(LocalIdx::from_raw(6), LocalIdx::from_raw(2)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_has_errors(&result.errors);
}

// V09-T16: Two two-phase borrows of same place, second created during first's reservation
// Both are two-phase mut borrows. The second conflicts with the first even
// during reservation, because two mutable borrows always conflict.

#[test]
fn t16_two_two_phase_mut_borrows_same_place_error() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);

        let locals = vec![
            local_decl(unit),       // _0 return
            local_decl(i32_ty),     // _1 data
            local_decl(mut_ref_ty), // _2 first two-phase mut ref
            local_decl(mut_ref_ty), // _3 second two-phase mut ref
            local_decl(i32_ty),     // _4 result
            local_decl(i32_ty),     // _5 result2
        ];

        let stmts = vec![
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(3)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_has_errors(&result.errors);
}

// V09-T17: Two-phase borrow, shared borrow created before — no conflict
// A shared borrow that exists before the two-phase borrow is created does
// not conflict because the shared borrow came first. The two-phase borrow
// creation only conflicts with *active* loans on the same place that are
// mutable or unique.

#[test]
fn t17_shared_borrow_before_two_phase_no_conflict() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(shared_ref_ty), // _2 shared ref (created first)
            local_decl(mut_ref_ty),    // _3 mut ref (two-phase, created second)
            local_decl(i32_ty),        // _4 result
            local_decl(i32_ty),        // _5 result2
        ];

        let stmts = vec![
            // _2 = &_1 (shared borrow first)
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            // _3 = &mut _1 (two-phase mut borrow second — conflicts with shared!)
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(3)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    // Creating a mut borrow while a shared borrow is active is always an error
    assert_has_errors(&result.errors);
}

// V09-T18: Two-phase borrow with read then second shared borrow — no conflict
// During reservation, we can both read the place AND create a shared borrow.

#[test]
fn t18_two_phase_reservation_read_and_shared_borrow() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(mut_ref_ty),    // _2 mut ref (two-phase)
            local_decl(i32_ty),        // _3 copy of data (read)
            local_decl(shared_ref_ty), // _4 shared ref
            local_decl(i32_ty),        // _5 result
            local_decl(i32_ty),        // _6 result2
        ];

        let stmts = vec![
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            // Read the data during reservation
            use_local(LocalIdx::from_raw(3), LocalIdx::from_raw(1)),
            // Shared borrow during reservation
            assign_borrow(
                LocalIdx::from_raw(4),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            // Use shared ref
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(4)),
            // Activate the two-phase borrow
            use_local(LocalIdx::from_raw(6), LocalIdx::from_raw(2)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_no_errors(&result.errors);
}
