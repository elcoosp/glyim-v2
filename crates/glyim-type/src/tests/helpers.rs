//! Local test helpers — standalone so we don't depend on glyim-test availability.

use crate::TyCtx;
use crate::TyCtxMut;
use glyim_core::interner::Interner;

pub fn test_ty_ctx() -> TyCtxMut {
    TyCtxMut::new(Interner::new())
}

pub fn test_frozen_ty_ctx() -> TyCtx {
    test_ty_ctx().freeze()
}

pub fn with_fresh_ty_ctx<F, R>(f: F) -> (TyCtx, R)
where
    F: FnOnce(&mut TyCtxMut) -> R,
{
    let mut ctx_mut = test_ty_ctx();
    let result = f(&mut ctx_mut);
    (ctx_mut.freeze(), result)
}
