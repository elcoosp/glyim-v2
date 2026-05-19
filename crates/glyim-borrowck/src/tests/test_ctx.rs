//! Shared test context implementing BorrowckCtx for testing.

use crate::BorrowckCtx;
use glyim_mir::{Body, LocalIdx, LocalDecl};
use glyim_type::TyCtx;

pub struct TestBorrowckCtx<'a> {
    ctx: &'a TyCtx,
    body: &'a Body,
}

impl<'a> TestBorrowckCtx<'a> {
    pub fn new(ctx: &'a TyCtx, body: &'a Body) -> Self {
        Self { ctx, body }
    }
}

impl<'a> BorrowckCtx for TestBorrowckCtx<'a> {
    fn ty_ctx(&self) -> &TyCtx {
        self.ctx
    }

    fn local_decl(&self, local: LocalIdx) -> &LocalDecl {
        &self.body.locals[local]
    }

    fn local_name(&self, local: LocalIdx) -> String {
        format!("_{}", local.to_raw())
    }
}
