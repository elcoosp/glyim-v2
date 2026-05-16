//! V33-T01: Generic function used from multiple call sites → mono items collected.
//!
//! Tests that the monomorphization pipeline correctly discovers root mono items
//! (main, #[no_mangle], etc.), collects transitive items through call graphs,
//! and partitions them into codegen units.

use glyim_core::def_id::{CrateId, DefId, FnDefId, LocalDefId};
use glyim_lower::mono::{MonoCtx, MonoItem};
use glyim_lower::partition::partition;
use glyim_mir::Body;
use glyim_type::Substitution;
use std::sync::Arc;

use crate::Pipeline;

/// Helper: build a MonoCtx with a single function mono item and a trivial
/// MIR body provider. The provider returns a dummy body for any DefId.
fn collect_with_dummy_provider(roots: &[MonoItem]) -> MonoCtx {
    let mut ctx = MonoCtx::new();
    let provider = |_def_id: DefId, _substs: &Substitution| -> Arc<Body> {
        Arc::new(Body::dummy(DefId::new(
            CrateId::from_raw(0),
            LocalDefId::from_raw(0),
        )))
    };
    let drop_provider = |_ty: glyim_type::Ty| -> Arc<Body> {
        Arc::new(Body::dummy(DefId::new(
            CrateId::from_raw(0),
            LocalDefId::from_raw(0),
        )))
    };
    ctx.collect(roots, &provider, &drop_provider);
    ctx
}

#[test]
fn mono_ctx_collects_single_root() {
    let empty_subst = Substitution::empty();
    let root = MonoItem::Fn {
        def_id: FnDefId::from_raw(0),
        substs: empty_subst,
    };
    let ctx = collect_with_dummy_provider(&[root.clone()]);

    assert_eq!(ctx.items().len(), 1, "should collect exactly one mono item");
    assert_eq!(
        ctx.items()[0].item,
        root,
        "collected item should match root"
    );
}

#[test]
fn mono_ctx_deduplicates_identical_roots() {
    let empty_subst = Substitution::empty();
    let root = MonoItem::Fn {
        def_id: FnDefId::from_raw(0),
        substs: empty_subst,
    };
    let ctx = collect_with_dummy_provider(&[root.clone(), root.clone()]);

    assert_eq!(
        ctx.items().len(),
        1,
        "duplicate roots should be deduplicated"
    );
}

#[test]
fn mono_ctx_collects_multiple_distinct_roots() {
    let empty_subst = Substitution::empty();
    let root_a = MonoItem::Fn {
        def_id: FnDefId::from_raw(0),
        substs: empty_subst,
    };
    let root_b = MonoItem::Fn {
        def_id: FnDefId::from_raw(1),
        substs: empty_subst,
    };
    let ctx = collect_with_dummy_provider(&[root_a, root_b]);

    assert_eq!(
        ctx.items().len(),
        2,
        "should collect two distinct mono items"
    );
}

#[test]
fn mono_ctx_static_item_collected() {
    use glyim_core::def_id::StaticDefId;
    let root = MonoItem::Static {
        def_id: StaticDefId::from_raw(0),
    };
    let ctx = collect_with_dummy_provider(&[root.clone()]);

    assert_eq!(ctx.items().len(), 1);
    match &ctx.items()[0].item {
        MonoItem::Static { def_id } => assert_eq!(def_id.to_raw(), 0),
        other => panic!("expected Static item, got {:?}", other),
    }
}

#[test]
fn partition_empty_items_returns_empty() {
    let result = partition(&[], 4);
    assert!(result.is_empty(), "empty input should produce empty CGUs");
}

#[test]
fn partition_single_item_one_cgu() {
    let empty_subst = Substitution::empty();
    let item = glyim_lower::mono::MonoItemData {
        item: MonoItem::Fn {
            def_id: FnDefId::from_raw(0),
            substs: empty_subst,
        },
        body: Arc::new(Body::dummy(DefId::new(
            CrateId::from_raw(0),
            LocalDefId::from_raw(0),
        ))),
        symbol: "test_fn".to_string(),
        source_module: 0,
    };
    let result = partition(&[item], 4);
    assert_eq!(result.len(), 1, "single item should produce one CGU");
    assert_eq!(result[0].len(), 1);
}

#[test]
fn partition_groups_by_source_module() {
    let empty_subst = Substitution::empty();
    let dummy_body = || {
        Arc::new(Body::dummy(DefId::new(
            CrateId::from_raw(0),
            LocalDefId::from_raw(0),
        )))
    };
    let items: Vec<glyim_lower::mono::MonoItemData> = vec![
        glyim_lower::mono::MonoItemData {
            item: MonoItem::Fn {
                def_id: FnDefId::from_raw(0),
                substs: empty_subst,
            },
            body: dummy_body(),
            symbol: "mod0_fn0".to_string(),
            source_module: 0,
        },
        glyim_lower::mono::MonoItemData {
            item: MonoItem::Fn {
                def_id: FnDefId::from_raw(1),
                substs: empty_subst,
            },
            body: dummy_body(),
            symbol: "mod0_fn1".to_string(),
            source_module: 0,
        },
        glyim_lower::mono::MonoItemData {
            item: MonoItem::Fn {
                def_id: FnDefId::from_raw(2),
                substs: empty_subst,
            },
            body: dummy_body(),
            symbol: "mod1_fn0".to_string(),
            source_module: 1,
        },
    ];
    let result = partition(&items, 4);
    assert_eq!(result.len(), 2, "two modules should produce two CGUs");
    let total: usize = result.iter().map(|g: &Vec<usize>| g.len()).sum();
    assert_eq!(total, 3, "all items should be assigned to a CGU");
}

#[test]
fn partition_merges_when_exceeds_max_cgus() {
    let empty_subst = Substitution::empty();
    let dummy_body = || {
        Arc::new(Body::dummy(DefId::new(
            CrateId::from_raw(0),
            LocalDefId::from_raw(0),
        )))
    };
    let items: Vec<glyim_lower::mono::MonoItemData> = (0..5)
        .map(|i| glyim_lower::mono::MonoItemData {
            item: MonoItem::Fn {
                def_id: FnDefId::from_raw(i),
                substs: empty_subst,
            },
            body: dummy_body(),
            symbol: format!("mod{}_fn", i),
            source_module: i,
        })
        .collect();
    let result = partition(&items, 2);
    assert!(
        result.len() <= 2,
        "should merge to at most max_cgus=2, got {}",
        result.len()
    );
    let total: usize = result.iter().map(|g: &Vec<usize>| g.len()).sum();
    assert_eq!(total, 5, "all items should be assigned to a CGU");
}

#[test]
fn full_pipeline_produces_mono_items() {
    use glyim_test::mock::{MockCodegen, TestDbBuilder};
    use std::io::Write;

    let source = "fn main() {}";
    let mut tmp = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    write!(tmp, "{}", source).expect("Failed to write temp file");
    let path = tmp.path().to_path_buf();
    let mut builder = TestDbBuilder::new()
        .name("test_mono")
        .target_triple("x86_64-unknown-linux-gnu")
        .opt_level(0);
    builder = builder.file(path.clone(), std::sync::Arc::from(source));
    let mut db = builder.build();
    let backend = MockCodegen::new();
    let output_path = std::path::Path::new("test_output.o");
    let result = Pipeline::compile_file(&mut db, &path, &backend, output_path);
    assert!(
        result.is_ok(),
        "pipeline should succeed: {:?}",
        result.err()
    );
}
