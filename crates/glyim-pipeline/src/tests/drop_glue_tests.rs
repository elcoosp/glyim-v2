use glyim_test::with_fresh_ty_ctx;
use glyim_core::primitives::IntTy;
use glyim_type::TyKind;

#[test]
fn drop_glue_for_i32_generates_body() {
    let (ty_ctx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I32)));
    let body = crate::mono_cache::generate_drop_glue(ty, &ty_ctx);
    // Just verify the body has at least one block (no panic)
    assert!(body.basic_blocks.len() >= 1);
}

#[test]
fn drop_glue_for_bool_generates_body() {
    let (ty_ctx, ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    let body = crate::mono_cache::generate_drop_glue(ty, &ty_ctx);
    assert!(body.basic_blocks.len() >= 1);
}

#[test]
fn drop_glue_for_never_type_generates_body() {
    let (ty_ctx, ty) = with_fresh_ty_ctx(|c| c.never_ty());
    let body = crate::mono_cache::generate_drop_glue(ty, &ty_ctx);
    assert!(body.basic_blocks.len() >= 1);
}
