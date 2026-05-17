//! Deduplication of monomorphized items based on polymorphized keys.

use glyim_mir;
use glyim_type::*;
use std::collections::HashSet;

use crate::mono::{MonoItem, MonoItemData};

use super::analyze::analyze_used_params;
use super::substitute::polymorphize_substs;

/// Compute the polymorphized version of a MonoItem.
///
/// For functions and constants, analyzes which generic parameters are used
/// and replaces unused ones with placeholders. Statics are returned unchanged.
///
/// This is the core of polymorphization: two MonoItems that differ only in
/// unused generic parameters will produce the same polymorphized MonoItem,
/// allowing them to be deduplicated.
pub fn compute_poly_item(ctx: &mut TyCtxMut, item: &MonoItem, body: &glyim_mir::Body) -> MonoItem {
    match item {
        MonoItem::Fn { def_id, substs } => {
            if substs.is_empty() {
                return item.clone();
            }
            let used = analyze_used_params(body, ctx, *substs);
            let poly_substs = polymorphize_substs(ctx, *substs, &used);
            MonoItem::Fn {
                def_id: *def_id,
                substs: poly_substs,
            }
        }
        MonoItem::Const { def_id, substs } => {
            if substs.is_empty() {
                return item.clone();
            }
            let used = analyze_used_params(body, ctx, *substs);
            let poly_substs = polymorphize_substs(ctx, *substs, &used);
            MonoItem::Const {
                def_id: *def_id,
                substs: poly_substs,
            }
        }
        MonoItem::Static { .. } => item.clone(),
        MonoItem::DropGlue { .. } => item.clone(),
    }
}

/// Deduplicate mono items based on polymorphized keys.
///
/// Items that differ only in unused generic parameters are merged into a
/// single item, reducing code size. The first occurrence of each polymorphized
/// key is kept; subsequent duplicates are dropped.
///
/// # Example
///
/// If `foo::<i32>()` and `foo::<bool>()` both have an unused type parameter `T`,
/// they will be deduplicated to a single `foo::<()>()` mono item.
pub fn deduplicate(ctx: &mut TyCtxMut, items: &[MonoItemData]) -> Vec<MonoItemData> {
    let mut seen: HashSet<MonoItem> = HashSet::new();
    let mut result = Vec::new();

    for data in items {
        let poly_item = compute_poly_item(ctx, &data.item, &data.body);
        if seen.contains(&poly_item) {
            continue;
        }
        seen.insert(poly_item.clone());
        result.push(MonoItemData {
            item: poly_item,
            body: data.body.clone(),
            symbol: data.symbol.clone(),
            source_module: data.source_module,
        });
    }

    result
}
