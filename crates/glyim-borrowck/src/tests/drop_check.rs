//! Tests for drop checking: use after drop, drop order, conditional drop flags.

use glyim_test::with_fresh_ty_ctx;
use glyim_core::arena::IndexVec;
use glyim_mir::{
    BasicBlockData, Body, LocalDecl, LocalIdx, Operand, Place, Rvalue,
    StatementKind, TerminatorKind, Statement, SourceInfo,
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

fn body_with_statements(stmts: Vec<Statement>) -> Body {
    let mut body = dummy_body();
    let mut locals = IndexVec::new();
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: glyim_core::Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: glyim_core::Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    locals.push(LocalDecl {
        ty: Ty::UNIT,
        mutability: glyim_core::Mutability::Not,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    body.locals = locals;

    let bb = BasicBlockData {
        statements: stmts,
        terminator: glyim_mir::Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    };
    let mut blocks = IndexVec::new();
    blocks.push(bb);
    body.basic_blocks = blocks;

    body
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn use_after_drop_error() {
    // V11-T01: using a value after it has been dropped should report an error.
    let (ctx, _) = with_fresh_ty_ctx(|_ctx_mut| {});
    let stmts = vec![
        glyim_mir::Statement {
            kind: StatementKind::StorageDead(LocalIdx::from_raw(0)),
            source_info: SourceInfo::new(Span::DUMMY),
        },
        glyim_mir::Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::Use(Operand::Move(Place::new(LocalIdx::from_raw(0)))),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        },
    ];
    let body = body_with_statements(stmts);
    let test_ctx = TestCtx::new(&ctx, &body.locals);
    let result = check_borrows(&test_ctx, &body);
    assert!(!result.errors.is_empty(), "Expected use-after-drop error");
}

#[test]
fn drop_order_mixed_fields() {
    // V11-T03: Dropping a struct with mixed fields should not cause false errors.
    // The test simply drops a local (representing a struct) and does not use it afterwards.
    let (ctx, _) = with_fresh_ty_ctx(|_ctx_mut| {});
    let stmts = vec![
        glyim_mir::Statement {
            kind: StatementKind::StorageDead(LocalIdx::from_raw(0)),
            source_info: SourceInfo::new(Span::DUMMY),
        },
    ];
    let body = body_with_statements(stmts);
    let test_ctx = TestCtx::new(&ctx, &body.locals);
    let result = check_borrows(&test_ctx, &body);
    assert!(result.errors.is_empty(), "dropping a local with no subsequent use should be ok");
}

#[test]
fn conditional_drop_flags() {
    // V11-T04: placeholder test for conditional drop flags.
    let (ctx, _) = with_fresh_ty_ctx(|_ctx_mut| {});
    let body = dummy_body();
    let test_ctx = TestCtx::new(&ctx, &body.locals);
    let result = check_borrows(&test_ctx, &body);
    assert!(result.errors.is_empty());
}
