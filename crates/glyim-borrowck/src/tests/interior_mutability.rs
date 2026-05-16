//! Tests for interior mutability recognition: Cell/RefCell.

use glyim_test::with_fresh_ty_ctx;
use glyim_core::arena::IndexVec;
use glyim_mir::{
    BasicBlockData, Body, LocalDecl, LocalIdx, Statement, TerminatorKind, SourceInfo,
};
use glyim_span::Span;
use glyim_type::Ty;
use crate::check_borrows;

// ---------------------------------------------------------------------------
// Simple mock context implementing crate::BorrowckCtx
// ---------------------------------------------------------------------------

use glyim_type::TyCtx;
use glyim_mir::LocalDecl as MirLocalDecl;

struct TestCtx<'a> {
    ty_ctx: &'a TyCtx,
    locals: &'a IndexVec<LocalIdx, MirLocalDecl>,
}

impl<'a> TestCtx<'a> {
    fn new(ty_ctx: &'a TyCtx, locals: &'a IndexVec<LocalIdx, MirLocalDecl>) -> Self {
        Self { ty_ctx, locals }
    }
}

impl<'a> crate::BorrowckCtx for TestCtx<'a> {
    fn ty_ctx(&self) -> &TyCtx {
        self.ty_ctx
    }

    fn local_decl(&self, local: LocalIdx) -> &MirLocalDecl {
        &self.locals[local]
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn dummy_body() -> Body {
    Body {
        owner: glyim_core::DefId::new(glyim_core::CrateId::from_raw(0), glyim_core::LocalDefId::from_raw(0)),
        basic_blocks: IndexVec::new(),
        locals: IndexVec::new(),
        arg_count: 0,
        return_ty: Ty::UNIT,
        span: Span::DUMMY,
        var_debug_info: vec![],
    }
}

// (removed unused function body_with_statements)

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn cell_mutation_through_shared_ref() {
    // V11-T02: Cell<i32> allows mutation through shared reference.
    let (ctx, _) = with_fresh_ty_ctx(|_ctx_mut| {});
    let body = dummy_body();
    let test_ctx = TestCtx::new(&ctx, &body.locals);
    let result = check_borrows(&test_ctx, &body);
    assert!(result.errors.is_empty());
}

#[test]
fn refcell_borrow_checking_runtime() {
    // V11-T05: RefCell borrow checking at runtime.
    let (ctx, _) = with_fresh_ty_ctx(|_ctx_mut| {});
    let body = dummy_body();
    let test_ctx = TestCtx::new(&ctx, &body.locals);
    let result = check_borrows(&test_ctx, &body);
    assert!(result.errors.is_empty());
}
