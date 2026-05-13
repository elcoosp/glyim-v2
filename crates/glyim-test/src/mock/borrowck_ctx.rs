use glyim_borrowck::BorrowckCtx;
use glyim_mir::{Body, LocalDecl, LocalIdx};
use glyim_type::{Ty, TyCtx};

pub struct MockBorrowckCtx<'a> {
    pub ty_ctx: &'a TyCtx,
    pub body: &'a Body,
}

impl<'a> MockBorrowckCtx<'a> {
    pub fn new(ty_ctx: &'a TyCtx, body: &'a Body) -> Self {
        Self { ty_ctx, body }
    }
}

impl BorrowckCtx for MockBorrowckCtx<'_> {
    fn ty_ctx(&self) -> &TyCtx {
        self.ty_ctx
    }
    fn local_decl(&self, local: LocalIdx) -> &LocalDecl {
        &self.body.locals[local]
    }
    fn is_copy(&self, _ty: Ty) -> bool {
        false
    }
}
