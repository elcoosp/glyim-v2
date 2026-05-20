#[test]
fn mono_ctx_new_is_empty() {
    let ctx = MonoCtx::new();
    assert_eq!(ctx.item_count(), 0);
    assert_eq!(ctx.cache_len(), 0);
    assert!(ctx.items().is_empty());
}

#[test]
fn mono_ctx_lookup_missing_returns_none() {
    let ctx = MonoCtx::new();
    let ty = glyim_test::test_frozen_ty_ctx().error_ty();
    let item = MonoItem::DropGlue { ty };
    assert!(ctx.lookup(&item).is_none());
}

#[test]
fn mono_ctx_polymorphize_empty_does_not_panic() {
    let mut ctx = MonoCtx::new();
    let mut ty_ctx = test_ty_ctx();
    ctx.polymorphize_and_deduplicate(&mut ty_ctx);
    assert_eq!(ctx.item_count(), 0);
    assert_eq!(ctx.cache_len(), 0);
}

#[test]
fn mono_ctx_default_is_empty() {
    let ctx: MonoCtx = Default::default();
    assert_eq!(ctx.item_count(), 0);
}
