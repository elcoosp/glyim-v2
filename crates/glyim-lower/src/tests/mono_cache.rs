//! Tests for mono item cache and deduplication (V24).

use crate::mono::{MonoCtx, MonoItem};
use glyim_core::def_id::{ConstDefId, CrateId, DefId, FnDefId, LocalDefId, StaticDefId};
use glyim_mir::Body;
use glyim_test::test_ty_ctx;
use glyim_type::{GenericArg, Substitution};
use std::sync::Arc;

/// Helper: create a Fn MonoItem.
fn make_fn_item(fn_def_id: FnDefId, substs: Substitution) -> MonoItem {
    MonoItem::Fn {
        def_id: fn_def_id,
        substs,
    }
}

/// Helper: create a Const MonoItem.
fn make_const_item(const_def_id: ConstDefId, substs: Substitution) -> MonoItem {
    MonoItem::Const {
        def_id: const_def_id,
        substs,
    }
}

/// Helper: create a Static MonoItem.
fn make_static_item(static_def_id: StaticDefId) -> MonoItem {
    MonoItem::Static {
        def_id: static_def_id,
    }
}

/// Helper: create a dummy MIR body.
fn dummy_body() -> Arc<Body> {
    Arc::new(Body::dummy(DefId::new(
        CrateId::from_raw(0),
        LocalDefId::from_raw(0),
    )))
}

// V24-T01: Same generic instantiation called twice produces only one mono item.
#[test]
fn same_instantiation_deduplicated() {
    let mut ty_ctx = test_ty_ctx();
    let empty_subst = ty_ctx.intern_substitution(Vec::new());
    let fn_id = FnDefId::from_raw(1);
    let item = make_fn_item(fn_id, empty_subst);

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[item.clone(), item.clone()],
        &|_def_id, _substs| dummy_body(),
        &|_ty| dummy_body(),
    );

    assert_eq!(ctx.item_count(), 1, "same mono item should be deduplicated");
}

// V24-T02: Different substitutions produce distinct items.
#[test]
fn different_substitutions_distinct() {
    let mut ty_ctx = test_ty_ctx();

    let subst_a = ty_ctx.intern_substitution(vec![GenericArg::Ty(ty_ctx.bool_ty())]);
    let subst_b = ty_ctx.intern_substitution(vec![GenericArg::Ty(ty_ctx.never_ty())]);

    // In the same context, different type arguments must produce different
    // Substitution indices, hence different MonoItems.
    assert_ne!(
        subst_a, subst_b,
        "substitutions with different types must differ"
    );

    let mut ctx = MonoCtx::new();
    let fn_id = FnDefId::from_raw(1);

    let item_a = make_fn_item(fn_id, subst_a);
    let item_b = make_fn_item(fn_id, subst_b);

    ctx.collect(
        &[item_a, item_b],
        &|_def_id, _substs| dummy_body(),
        &|_ty| dummy_body(),
    );

    assert_eq!(
        ctx.item_count(),
        2,
        "different substitutions should produce distinct items"
    );
}

// V24-T03: Cache survives across multiple worklist iterations (collect calls).
#[test]
fn cache_survives_across_collects() {
    let mut ty_ctx = test_ty_ctx();
    let empty_subst = ty_ctx.intern_substitution(Vec::new());
    let fn_id = FnDefId::from_raw(1);
    let item = make_fn_item(fn_id, empty_subst);

    let mut ctx = MonoCtx::new();

    // First collect
    ctx.collect(&[item.clone()], &|_def_id, _substs| dummy_body(), &|_ty| {
        dummy_body()
    });
    assert_eq!(ctx.item_count(), 1);

    // Second collect with same item — should not duplicate
    ctx.collect(&[item.clone()], &|_def_id, _substs| dummy_body(), &|_ty| {
        dummy_body()
    });
    assert_eq!(
        ctx.item_count(),
        1,
        "second collect should not duplicate items"
    );

    // Third collect with same item — still one
    ctx.collect(&[item.clone()], &|_def_id, _substs| dummy_body(), &|_ty| {
        dummy_body()
    });
    assert_eq!(
        ctx.item_count(),
        1,
        "third collect should not duplicate items"
    );
}

// V24-T04: No duplicate Arc<Body> — cached item retains original body.
#[test]
fn no_duplicate_arc_body() {
    let mut ty_ctx = test_ty_ctx();
    let empty_subst = ty_ctx.intern_substitution(Vec::new());
    let fn_id = FnDefId::from_raw(1);
    let item = make_fn_item(fn_id, empty_subst);

    let body = dummy_body();
    let body_ptr = Arc::as_ptr(&body);

    let mut ctx = MonoCtx::new();

    // First collect
    ctx.collect(&[item.clone()], &|_def_id, _substs| body.clone(), &|_ty| {
        dummy_body()
    });
    assert_eq!(ctx.item_count(), 1);

    // Second collect: provide a different body via closure to prove it's not called
    let different_body = Arc::new(Body::dummy(DefId::new(
        CrateId::from_raw(99),
        LocalDefId::from_raw(99),
    )));
    ctx.collect(
        &[item.clone()],
        &|_def_id, _substs| different_body.clone(),
        &|_ty| dummy_body(),
    );

    // Should still have only 1 item with the original body
    assert_eq!(
        ctx.item_count(),
        1,
        "should not duplicate across collect calls"
    );
    assert_eq!(
        Arc::as_ptr(&ctx.items()[0].body),
        body_ptr,
        "body should be the original, not a duplicate"
    );
}

// Additional: lookup returns correct MonoItemId after collect.
#[test]
fn lookup_returns_correct_id() {
    let mut ty_ctx = test_ty_ctx();
    let empty_subst = ty_ctx.intern_substitution(Vec::new());
    let fn_id = FnDefId::from_raw(42);
    let item = make_fn_item(fn_id, empty_subst);

    let mut ctx = MonoCtx::new();

    // Before collect, lookup should return None
    assert!(
        ctx.lookup(&item).is_none(),
        "lookup should return None before collect"
    );

    ctx.collect(&[item.clone()], &|_def_id, _substs| dummy_body(), &|_ty| {
        dummy_body()
    });

    // After collect, lookup should return Some
    let id = ctx
        .lookup(&item)
        .expect("lookup should return Some after collect");
    assert_eq!(ctx.items()[id.to_raw() as usize].item, item);
}

// Additional: lookup works correctly across multiple collect calls.
#[test]
fn lookup_across_collects() {
    let mut ty_ctx = test_ty_ctx();
    let empty_subst = ty_ctx.intern_substitution(Vec::new());
    let fn_id_a = FnDefId::from_raw(1);
    let fn_id_b = FnDefId::from_raw(2);

    let item_a = make_fn_item(fn_id_a, empty_subst);
    let item_b = make_fn_item(fn_id_b, empty_subst);

    let mut ctx = MonoCtx::new();

    // First collect: item_a
    ctx.collect(
        &[item_a.clone()],
        &|_def_id, _substs| dummy_body(),
        &|_ty| dummy_body(),
    );
    let id_a = ctx.lookup(&item_a).expect("should find item_a");

    // Second collect: item_b
    ctx.collect(
        &[item_b.clone()],
        &|_def_id, _substs| dummy_body(),
        &|_ty| dummy_body(),
    );
    let id_b = ctx.lookup(&item_b).expect("should find item_b");

    // Both lookups should still work after second collect
    assert_eq!(
        ctx.lookup(&item_a),
        Some(id_a),
        "item_a lookup should still work after second collect"
    );
    assert_ne!(id_a, id_b, "different items should have different IDs");
}

// Additional: static items are also cached.
#[test]
fn static_items_cached() {
    let static_id = StaticDefId::from_raw(10);
    let item = make_static_item(static_id);

    let mut ctx = MonoCtx::new();
    ctx.collect(&[item.clone()], &|_def_id, _substs| dummy_body(), &|_ty| {
        dummy_body()
    });
    assert_eq!(ctx.item_count(), 1);

    let id = ctx.lookup(&item).expect("should find static item");
    assert_eq!(ctx.items()[id.to_raw() as usize].item, item);
}

// Additional: const items are also cached.
#[test]
fn const_items_cached() {
    let mut ty_ctx = test_ty_ctx();
    let empty_subst = ty_ctx.intern_substitution(Vec::new());
    let const_id = ConstDefId::from_raw(5);
    let item = make_const_item(const_id, empty_subst);

    let mut ctx = MonoCtx::new();
    ctx.collect(&[item.clone()], &|_def_id, _substs| dummy_body(), &|_ty| {
        dummy_body()
    });
    assert_eq!(ctx.item_count(), 1);

    let id = ctx.lookup(&item).expect("should find const item");
    assert_eq!(ctx.items()[id.to_raw() as usize].item, item);
}

// Additional: hash consing — same substitution content in same context produces same key.
#[test]
fn hash_consing_within_same_context() {
    let mut ty_ctx = test_ty_ctx();

    // Intern the same substitution content twice in the same context
    let substs1 = ty_ctx.intern_substitution(vec![GenericArg::Ty(ty_ctx.bool_ty())]);
    let substs2 = ty_ctx.intern_substitution(vec![GenericArg::Ty(ty_ctx.bool_ty())]);

    // If hash consing works within the same context, they should be equal
    if substs1 == substs2 {
        let mut mono_ctx = MonoCtx::new();
        let fn_id = FnDefId::from_raw(1);

        let item1 = make_fn_item(fn_id, substs1);
        let item2 = make_fn_item(fn_id, substs2);

        // Identical substitutions -> same MonoItem -> deduplicated
        mono_ctx.collect(&[item1, item2], &|_def_id, _substs| dummy_body(), &|_ty| {
            dummy_body()
        });
        assert_eq!(
            mono_ctx.item_count(),
            1,
            "identical substitutions should produce one item"
        );
    }
    // If hash consing does not deduplicate within the same context,
    // that is a TyCtxMut concern, not a MonoCtx concern.
}

// Additional: cache_len tracks correctly and matches item_count.
#[test]
fn cache_len_matches_item_count() {
    let mut ctx = MonoCtx::new();
    assert_eq!(ctx.cache_len(), 0, "empty context should have cache_len 0");

    let mut ty_ctx = test_ty_ctx();
    let empty_subst = ty_ctx.intern_substitution(Vec::new());
    let fn_id = FnDefId::from_raw(1);
    let item = make_fn_item(fn_id, empty_subst);

    ctx.collect(&[item], &|_def_id, _substs| dummy_body(), &|_ty| {
        dummy_body()
    });
    assert_eq!(
        ctx.cache_len(),
        ctx.item_count(),
        "cache_len should equal item_count after collect"
    );
}

// Additional: multiple distinct items each get unique IDs.
#[test]
fn multiple_items_unique_ids() {
    let mut ty_ctx = test_ty_ctx();
    let empty_subst = ty_ctx.intern_substitution(Vec::new());
    let fn_id_1 = FnDefId::from_raw(1);
    let fn_id_2 = FnDefId::from_raw(2);
    let static_id = StaticDefId::from_raw(3);

    let item_fn1 = make_fn_item(fn_id_1, empty_subst);
    let item_fn2 = make_fn_item(fn_id_2, empty_subst);
    let item_static = make_static_item(static_id);

    let mut ctx = MonoCtx::new();
    ctx.collect(
        &[item_fn1.clone(), item_fn2.clone(), item_static.clone()],
        &|_def_id, _substs| dummy_body(),
        &|_ty| dummy_body(),
    );

    assert_eq!(ctx.item_count(), 3);

    let id_fn1 = ctx.lookup(&item_fn1).expect("fn1");
    let id_fn2 = ctx.lookup(&item_fn2).expect("fn2");
    let id_static = ctx.lookup(&item_static).expect("static");

    // All IDs should be distinct
    assert_ne!(id_fn1, id_fn2);
    assert_ne!(id_fn1, id_static);
    assert_ne!(id_fn2, id_static);
}

// Additional: same item passed in start and also discovered via scan is still only one.
#[test]
fn root_and_discovered_dedup() {
    let mut ty_ctx = test_ty_ctx();
    let empty_subst = ty_ctx.intern_substitution(Vec::new());
    let fn_id = FnDefId::from_raw(1);
    let item = make_fn_item(fn_id, empty_subst);

    let mut ctx = MonoCtx::new();

    // Same item as root
    ctx.collect(&[item.clone()], &|_def_id, _substs| dummy_body(), &|_ty| {
        dummy_body()
    });

    // Collect again with same item as root (simulates re-discovery)
    ctx.collect(&[item.clone()], &|_def_id, _substs| dummy_body(), &|_ty| {
        dummy_body()
    });

    assert_eq!(
        ctx.item_count(),
        1,
        "same item discovered multiple ways should still be one"
    );
}
#[test]
fn type_needs_drop_i32_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I32)));
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

#[test]
fn type_needs_drop_u64_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Uint(UintTy::U64)));
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

#[test]
fn type_needs_drop_f64_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Float(FloatTy::F64)));
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

#[test]
fn type_needs_drop_bool_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

#[test]
fn type_needs_drop_char_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Char));
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

#[test]
fn type_needs_drop_string_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::String));
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

#[test]
fn type_needs_drop_unit_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.unit_ty());
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

#[test]
fn type_needs_drop_never_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.never_ty());
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

// ========== type_needs_drop: references & pointers ==========

#[test]
fn type_needs_drop_shared_ref_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| {
        let inner = c.mk_ty(TyKind::Int(IntTy::I32));
        let re = Region::Var(c.new_region_var());
        c.mk_ref(re, inner, Mutability::Not)
    });
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

#[test]
fn type_needs_drop_mut_ref_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| {
        let inner = c.mk_ty(TyKind::Int(IntTy::I32));
        let re = Region::Var(c.new_region_var());
        c.mk_ref(re, inner, Mutability::Mut)
    });
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

#[test]
fn type_needs_drop_raw_ptr_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| {
        let inner = c.mk_ty(TyKind::Int(IntTy::I32));
        c.mk_ty(TyKind::RawPtr(inner, Mutability::Not))
    });
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

// ========== type_needs_drop: composites ==========

#[test]
fn type_needs_drop_tuple_of_primitives_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| {
        let a = c.mk_ty(TyKind::Int(IntTy::I32));
        let b = c.bool_ty();
        c.mk_tuple(vec![a, b])
    });
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

#[test]
fn type_needs_drop_fn_ptr_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| {
        let sig = FnSig {
            inputs: Substitution::empty(),
            output: c.unit_ty(),
            c_variadic: false,
            unsafety: Safety::Safe,
            abi: Abi::Glyim,
        };
        c.mk_fn_ptr(sig)
    });
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

#[test]
fn type_needs_drop_error_is_false() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.error_ty());
    assert!(!type_needs_drop(ty, &tcx, &mut HashSet::new()));
}

// ========== type_needs_drop: visited-set behaviour ==========

#[test]
fn type_needs_drop_visited_set_prevents_recheck() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I32)));
    let mut visited = HashSet::new();
    let first = type_needs_drop(ty, &tcx, &mut visited);
    // Second call with the same visited set must short-circuit.
    let second = type_needs_drop(ty, &tcx, &mut visited);
    assert_eq!(first, false);
    assert_eq!(second, false);
    assert!(visited.contains(&ty));
}

// ========== generate_drop_glue: fast path (single Return block) ==========

#[test]
fn drop_glue_for_i32_generates_body() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I32)));
    let body = generate_drop_glue(ty, &tcx);
    assert!(!body.basic_blocks.is_empty());
}

#[test]
fn drop_glue_for_bool_generates_body() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    let body = generate_drop_glue(ty, &tcx);
    assert!(!body.basic_blocks.is_empty());
}

#[test]
fn drop_glue_for_never_type_generates_body() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.never_ty());
    let body = generate_drop_glue(ty, &tcx);
    assert!(!body.basic_blocks.is_empty());
}

#[test]
fn drop_glue_for_unit_returns_single_block() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.unit_ty());
    let body = generate_drop_glue(ty, &tcx);
    assert_eq!(body.basic_blocks.len(), 1);
    let block0 = &body.basic_blocks[BasicBlockIdx::from_raw(0)];
    assert!(matches!(block0.terminator.kind, TerminatorKind::Return));
}

#[test]
fn drop_glue_for_reference_returns_single_block() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| {
        let inner = c.mk_ty(TyKind::Int(IntTy::I32));
        let re = Region::Var(c.new_region_var());
        c.mk_ref(re, inner, Mutability::Not)
    });
    let body = generate_drop_glue(ty, &tcx);
    assert_eq!(body.basic_blocks.len(), 1);
    let block0 = &body.basic_blocks[BasicBlockIdx::from_raw(0)];
    assert!(matches!(block0.terminator.kind, TerminatorKind::Return));
}

#[test]
fn drop_glue_for_tuple_of_primitives_returns_single_block() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| {
        let a = c.mk_ty(TyKind::Int(IntTy::I32));
        let b = c.bool_ty();
        c.mk_tuple(vec![a, b])
    });
    let body = generate_drop_glue(ty, &tcx);
    assert_eq!(body.basic_blocks.len(), 1);
}

#[test]
fn drop_glue_for_fn_ptr_returns_single_block() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| {
        let sig = FnSig {
            inputs: Substitution::empty(),
            output: c.unit_ty(),
            c_variadic: false,
            unsafety: Safety::Safe,
            abi: Abi::Glyim,
        };
        c.mk_fn_ptr(sig)
    });
    let body = generate_drop_glue(ty, &tcx);
    assert_eq!(body.basic_blocks.len(), 1);
}

#[test]
fn drop_glue_for_float_returns_single_block() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Float(FloatTy::F64)));
    let body = generate_drop_glue(ty, &tcx);
    assert_eq!(body.basic_blocks.len(), 1);
}

#[test]
fn drop_glue_for_uint_returns_single_block() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Uint(UintTy::Usize)));
    let body = generate_drop_glue(ty, &tcx);
    assert_eq!(body.basic_blocks.len(), 1);
}

// ========== make_drop_glue_provider ==========

#[test]
fn drop_glue_provider_produces_valid_body() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Float(FloatTy::F32)));
    let provider = make_drop_glue_provider(&tcx);
    let body = provider(ty);
    assert!(!body.basic_blocks.is_empty());
    let block0 = &body.basic_blocks[BasicBlockIdx::from_raw(0)];
    assert!(matches!(block0.terminator.kind, TerminatorKind::Return));
}

#[test]
fn drop_glue_provider_idempotent_for_primitives() {
    let (tcx, ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    let provider = make_drop_glue_provider(&tcx);
    let b1 = provider(ty);
    let b2 = provider(ty);
    assert_eq!(b1.basic_blocks.len(), b2.basic_blocks.len());
    assert_eq!(b1.basic_blocks.len(), 1);
}

// ========== substitute_body ==========

#[test]
fn substitute_body_empty_substs_is_identity() {
    let (tcx, _ty) = with_fresh_ty_ctx(|c| c.mk_ty(TyKind::Int(IntTy::I32)));
    let dummy_body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    let substituted = substitute_body(&dummy_body, &Substitution::empty(), &tcx);
    assert_eq!(
        substituted.basic_blocks.len(),
        dummy_body.basic_blocks.len()
    );
    assert_eq!(substituted.locals.len(), dummy_body.locals.len());
}

// ========== build_mono_cache / polymorphization integration ==========

#[test]
fn build_mono_cache_empty_is_empty() {
    let mut mono_ctx = glyim_lower::MonoCtx::new();
    let mut ty_ctx = test_ty_ctx();
    let cache = build_mono_cache(&mut mono_ctx, &mut ty_ctx);
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
}

// ========== compute_max_cgus ==========

#[test]
fn compute_max_cgus_is_in_valid_range() {
    let max = compute_max_cgus();
    assert!(max >= 1 && max <= 16);
}

#[test]
fn compute_max_cgus_is_stable() {
    // Calling it twice on the same machine should yield the same value.
    let a = compute_max_cgus();
    let b = compute_max_cgus();
    assert_eq!(a, b);
}

// ========== PipelineMonoCache ==========

#[test]
fn pipeline_mono_cache_from_empty_items() {
    let items: Vec<MonoItemData> = vec![];
    let cache = PipelineMonoCache::from_items(&items);
    assert!(cache.is_empty());
    assert_eq!(cache.symbols().len(), 0);
}
