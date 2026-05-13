use glyim_type::{TyCtxMut, TyCtx};
pub fn test_ty_ctx() -> TyCtxMut {
    let interner = glyim_core::interner::Interner::new();
    TyCtxMut::new(interner)
}
pub fn test_frozen_ty_ctx() -> TyCtx {
    test_ty_ctx().freeze()
}
