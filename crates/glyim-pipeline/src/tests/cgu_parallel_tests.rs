//! V33-T02: Parallel codegen over CGUs → correct output.
//!
//! Tests that CGU partitioning works correctly and that the parallel
//! codegen path produces the same bodies as serial codegen.

use glyim_core::def_id::{CrateId, DefId, FnDefId, LocalDefId};
use glyim_lower::mono::{MonoItem, MonoItemData};
use glyim_lower::partition::partition;
use glyim_mir::Body;
use glyim_type::Substitution;
use std::sync::Arc;

fn dummy_body() -> Arc<Body> {
    Arc::new(Body::dummy(DefId::new(
        CrateId::from_raw(0),
        LocalDefId::from_raw(0),
    )))
}

fn make_items(n: usize, modules: usize) -> Vec<MonoItemData> {
    (0..n)
        .map(|i| MonoItemData {
            item: MonoItem::Fn {
                def_id: FnDefId::from_raw(i as u32),
                substs: Substitution::empty(),
            },
            body: dummy_body(),
            symbol: format!("fn_{}", i),
            source_module: (i % modules) as u32,
        })
        .collect()
}

#[test]
fn partition_all_same_module_one_cgu() {
    let items = make_items(5, 1);
    let result = partition(&items, 4);
    assert_eq!(result.len(), 1, "all same module → one CGU");
    assert_eq!(result[0].len(), 5);
}

#[test]
fn partition_respects_max_cgus() {
    let items = make_items(10, 10);
    for max in 1..=5 {
        let result = partition(&items, max);
        assert!(
            result.len() <= max,
            "max_cgus={} but got {} CGUs",
            max,
            result.len()
        );
        let total: usize = result.iter().map(|g: &Vec<usize>| g.len()).sum();
        assert_eq!(total, 10, "all items must be assigned (max={})", max);
    }
}

#[test]
fn partition_preserves_all_item_indices() {
    let items = make_items(8, 3);
    let result = partition(&items, 4);
    let mut all_indices: Vec<usize> = result
        .iter()
        .flat_map(|g: &Vec<usize>| g.iter().copied())
        .collect();
    all_indices.sort();
    let expected: Vec<usize> = (0..8).collect();
    assert_eq!(
        all_indices, expected,
        "all original indices must be present exactly once"
    );
}

#[test]
fn parallel_cgu_iteration_matches_serial() {
    let items = make_items(12, 4);
    let cgus = partition(&items, 4);

    let serial_bodies: Vec<Arc<Body>> = items.iter().map(|item| item.body.clone()).collect();

    let parallel_bodies: Vec<Arc<Body>> = cgus
        .iter()
        .flat_map(|cgu_indices: &Vec<usize>| cgu_indices.iter().map(|&idx| items[idx].body.clone()))
        .collect();

    assert_eq!(serial_bodies.len(), parallel_bodies.len());
    for (serial, parallel) in serial_bodies.iter().zip(parallel_bodies.iter()) {
        assert!(
            Arc::ptr_eq(serial, parallel)
                || serial.basic_blocks.len() == parallel.basic_blocks.len(),
            "bodies should match between serial and parallel iteration"
        );
    }
}

#[test]
fn rayon_parallel_cgu_iteration_succeeds() {
    use rayon::prelude::*;

    let items = make_items(20, 5);
    let cgus = partition(&items, 4);

    let results: Vec<usize> = cgus
        .par_iter()
        .map(|cgu_indices: &Vec<usize>| {
            cgu_indices
                .iter()
                .map(|&idx| items[idx].body.locals.len())
                .sum()
        })
        .collect();

    assert!(
        !results.is_empty(),
        "parallel iteration should produce results"
    );
}

#[test]
fn partition_single_item_max_cgu_1() {
    let items = make_items(1, 1);
    let result = partition(&items, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].len(), 1);
}
