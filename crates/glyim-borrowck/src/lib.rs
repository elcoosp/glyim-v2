//! Borrow checker using non-lexical lifetimes (NLL).
//!
//! This is a minimal implementation for v0.1.0.
//! It currently only reports errors via a stub.

use glyim_mir::Body;
use glyim_diag::GlyimDiagnostic;

#[derive(Clone, Debug)]
pub struct BorrowckResult {
    pub errors: Vec<GlyimDiagnostic>,
}

pub trait BorrowckCtx {
    fn ty_ctx(&self) -> &glyim_type::TyCtx;
    fn local_decl(&self, local: glyim_mir::LocalIdx) -> &glyim_mir::LocalDecl;
    fn is_copy(&self, ty: glyim_type::Ty) -> bool;
}

pub fn check_borrows(ctx: &dyn BorrowckCtx, body: &Body) -> BorrowckResult {
    // Stub: no actual analysis yet
    let _ = (ctx, body);
    BorrowckResult { errors: Vec::new() }
}
