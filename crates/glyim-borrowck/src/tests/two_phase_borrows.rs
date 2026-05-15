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

// ---------------------------------------------------------------------------
// V09-T01: vec.push(vec.len()) compiles (run-pass)
//
// Pattern: a two-phase mutable borrow is created, then a shared read
// of the same place occurs before the mutable reference is used (activation).
// This should NOT produce an error.
// ---------------------------------------------------------------------------

#[test]
fn t01_two_phase_borrow_with_intervening_shared_read() {
    // MIR equivalent of: vec.push(vec.len())
    //
    // _1 = &mut vec   (two-phase mutable borrow, reservation)
    // _2 = vec.len()  (shared read of vec during reservation phase)
    // _3 = Vec::push(_1, _2)  (activation of the mutable borrow)
    //
    // In a single-block MIR:
    //   _3 = &mut _1_place  (two-phase mut borrow of _1_place)
    //   _4 = *_1_place      (shared read during reservation)
    //   use(_3)             (activation)

    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| {
        let unit = ctx_mut.unit_ty();
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
        let mut_ref_ty = make_ref_ty(ctx_mut, i32_ty, true);
        let shared_ref_ty = make_ref_ty(ctx_mut, i32_ty, false);

        // locals: 0=return, 1=data, 2=mut_ref, 3=shared_ref, 4=result
        let locals = vec![
            local_decl(unit),          // _0 return
            local_decl(i32_ty),        // _1 data
            local_decl(mut_ref_ty),    // _2 mut ref (two-phase)
            local_decl(shared_ref_ty), // _3 shared ref
            local_decl(i32_ty),        // _4 result
        ];

        let stmts = vec![
            // _2 = &mut _1 (two-phase mutable borrow — reservation)
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            // _3 = &_1 (shared borrow of same place — allowed during reservation)
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            // _4 = *_3 (use the shared ref)
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)),
            // use _2 (activation: the mut ref is now live)
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_no_errors(&result.errors);
}

// ---------------------------------------------------------------------------
// V09-T02: Two-phase borrow with intervening shared borrow (run-pass)
//
// Similar to T01 but the intervening access is a full shared borrow,
// not just a read. During the reservation phase, a shared borrow of
// the same place should be allowed.
// ---------------------------------------------------------------------------

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
            // _2 = &mut _1 (two-phase — reservation only)
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            // _3 = &_1 (shared borrow during reservation — OK)
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            // use _3 (shared ref used and dies)
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)),
            // use _2 (activation of two-phase borrow)
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_no_errors(&result.errors);
}

// ---------------------------------------------------------------------------
// V09-T03: Activation point conflict detection → error (compile-fail)
//
// A two-phase mutable borrow is activated (used), then the same place
// is read while the mutable reference is still live. This should error
// because after activation, the mutable borrow is fully active.
// ---------------------------------------------------------------------------

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
            // _2 = &mut _1 (two-phase — reservation)
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            // use _2 (activation — mutable borrow is now fully active)
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
            // _3 = &_1 (shared borrow AFTER activation — conflict!)
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            // use _3 and _2
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(3)),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(2)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_has_errors(&result.errors);
}

// ---------------------------------------------------------------------------
// V09-T04: Reservation conflict with mutable borrow → error (compile-fail)
//
// Even during the reservation phase, a two-phase mutable borrow conflicts
// with another *mutable* borrow of the same place.
// ---------------------------------------------------------------------------

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
            // _2 = &mut _1 (two-phase — reservation)
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            // _3 = &mut _1 (another mutable borrow — conflicts even during reservation!)
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            // use both mut refs so they are live
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(3)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_has_errors(&result.errors);
}

// ---------------------------------------------------------------------------
// V09-T05: One-phase mutable borrow conflicts with shared (baseline)
//
// A regular (non-two-phase) mutable borrow should still conflict with
// a shared borrow of the same place, as before.
// ---------------------------------------------------------------------------

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
            // _2 = &mut _1 (one-phase — fully active immediately)
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_one_phase_mut(),
            ),
            // _3 = &_1 (shared borrow — conflicts with active mutable!)
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            // use both
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(2)),
            use_local(LocalIdx::from_raw(5), LocalIdx::from_raw(3)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_has_errors(&result.errors);
}

// ---------------------------------------------------------------------------
// V09-T06: Two-phase borrow where reservation is never activated
//
// If a two-phase mutable borrow is created but the reference is never
// used (never activated), it should not conflict with a shared borrow.
// This is because an unused mutable borrow is effectively dead.
// ---------------------------------------------------------------------------

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
            // _2 = &mut _1 (two-phase — reservation, but never used)
            assign_borrow(
                LocalIdx::from_raw(2),
                Place::new(LocalIdx::from_raw(1)),
                make_two_phase_mut(),
            ),
            // _3 = &_1 (shared borrow — OK because _2 is dead)
            assign_borrow(
                LocalIdx::from_raw(3),
                Place::new(LocalIdx::from_raw(1)),
                BorrowKind::Shared,
            ),
            // use _3 only
            use_local(LocalIdx::from_raw(4), LocalIdx::from_raw(3)),
        ];

        make_body(stmts, locals)
    });

    let mock = LocalMockBorrowckCtx { ty_ctx: ctx, body };
    let result = check_borrows(&mock, &mock.body);
    assert_no_errors(&result.errors);
}
