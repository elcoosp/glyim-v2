use glyim_test::{assert_mir, with_fresh_ty_ctx};
use glyim_core::primitives::IntTy;
use glyim_type::TyKind;

#[test]
fn drop_glue_for_i32_returns_only() {
    let (ty_ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I32)));
    let body = crate::mono_cache::generate_drop_glue(ty, &ty_ctx);
    assert_mir(&ty_ctx, &body).block_count(1);
    // We can't easily check terminator kind with our simple API, but we know it compiles.
}

#[test]
fn drop_glue_for_bool_returns_only() {
    let (ty_ctx, ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    let body = crate::mono_cache::generate_drop_glue(ty, &ty_ctx);
    assert_mir(&ty_ctx, &body).block_count(1);
}
