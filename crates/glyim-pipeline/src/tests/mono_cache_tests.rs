//! V33-T03: Cached mono items reused across compilations.
//!
//! Tests that MonoCtx correctly caches previously collected items and
//! doesn't re-collect them on subsequent passes.

use glyim_core::def_id::{CrateId, DefId, FnDefId, LocalDefId};
use glyim_lower::mono::{MonoCtx, MonoItem};
use glyim_mir::Body;
use glyim_type::Substitution;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

fn dummy_provider(_def_id: DefId, _substs: &Substitution) -> Arc<Body> {
    Arc::new(Body::dummy(DefId::new(
        CrateId::from_raw(0),
        LocalDefId::from_raw(0),
    )))
}

fn dummy_drop_provider(_ty: glyim_type::Ty) -> Arc<Body> {
    Arc::new(Body::dummy(DefId::new(
        CrateId::from_raw(0),
        LocalDefId::from_raw(0),
    )))
}

#[test]
fn mono_ctx_second_collect_skips_existing() {
    let empty_subst = Substitution::empty();
    let root = MonoItem::Fn {
        def_id: FnDefId::from_raw(0),
        substs: empty_subst,
    };
    let mut ctx = MonoCtx::new();
    ctx.collect(&[root.clone()], &dummy_provider, &dummy_drop_provider);
    assert_eq!(ctx.items().len(), 1);

    ctx.collect(&[root.clone()], &dummy_provider, &dummy_drop_provider);
    assert_eq!(
        ctx.items().len(),
        1,
        "second collect should not add duplicates"
    );
}

#[test]
fn mono_ctx_adds_new_items_on_second_collect() {
    let empty_subst = Substitution::empty();
    let root_a = MonoItem::Fn {
        def_id: FnDefId::from_raw(0),
        substs: empty_subst,
    };
    let root_b = MonoItem::Fn {
        def_id: FnDefId::from_raw(1),
        substs: empty_subst,
    };
    let mut ctx = MonoCtx::new();
    ctx.collect(&[root_a], &dummy_provider, &dummy_drop_provider);
    assert_eq!(ctx.items().len(), 1);

    ctx.collect(&[root_b], &dummy_provider, &dummy_drop_provider);
    assert_eq!(
        ctx.items().len(),
        2,
        "new root should be added on second collect"
    );
}

#[test]
fn mono_ctx_default_is_empty() {
    let ctx = MonoCtx::new();
    assert!(ctx.items().is_empty());
}

#[test]
fn mono_ctx_cache_prevents_re_entry() {
    let empty_subst = Substitution::empty();
    let root = MonoItem::Fn {
        def_id: FnDefId::from_raw(42),
        substs: empty_subst,
    };
    let mut ctx = MonoCtx::new();

    let call_count = Arc::new(AtomicU32::new(0));
    let call_count_clone = call_count.clone();
    let counting_provider = move |def_id: DefId, _substs: &Substitution| -> Arc<Body> {
        call_count_clone.fetch_add(1, Ordering::SeqCst);
        assert_eq!(
            def_id.local_id.to_raw(),
            42,
            "provider should only be called for def_id 42"
        );
        Arc::new(Body::dummy(DefId::new(
            CrateId::from_raw(0),
            LocalDefId::from_raw(0),
        )))
    };

    ctx.collect(&[root.clone()], &counting_provider, &dummy_drop_provider);
    let first_count = call_count.load(Ordering::SeqCst);
    assert!(
        first_count >= 1,
        "provider should have been called at least once"
    );

    ctx.collect(&[root], &counting_provider, &dummy_drop_provider);
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        first_count,
        "provider should not be called again for cached item"
    );
}

#[test]
fn mono_ctx_handles_mixed_fn_and_static() {
    use glyim_core::def_id::StaticDefId;
    let empty_subst = Substitution::empty();
    let fn_root = MonoItem::Fn {
        def_id: FnDefId::from_raw(0),
        substs: empty_subst,
    };
    let static_root = MonoItem::Static {
        def_id: StaticDefId::from_raw(1),
    };
    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[fn_root, static_root],
        &dummy_provider,
        &dummy_drop_provider,
    );
    assert_eq!(
        ctx.items().len(),
        2,
        "should collect both fn and static items"
    );
}
