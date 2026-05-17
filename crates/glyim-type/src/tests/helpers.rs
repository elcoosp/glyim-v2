use crate::*;
use glyim_core::interner::Interner;

pub(crate) fn test_ty_ctx() -> TyCtxMut {
    TyCtxMut::new(Interner::default())
}

pub(crate) fn test_frozen_ty_ctx() -> TyCtx {
    test_ty_ctx().freeze()
}

pub(crate) fn with_fresh_ty_ctx<F, R>(f: F) -> (TyCtx, R)
where
    F: FnOnce(&mut TyCtxMut) -> R,
{
    let mut ctx_mut = test_ty_ctx();
    let result = f(&mut ctx_mut);
    (ctx_mut.freeze(), result)
}
