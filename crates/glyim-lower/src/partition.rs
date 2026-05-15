//! Codegen unit partitioning.
//!
//! Groups monomorphized items into codegen units (CGUs) for parallel code generation.
//! Strategy: group by source module, then if number of module groups exceeds max_cgus,
//! merge the smallest groups into the largest ones.

use crate::mono::MonoItemData;
use std::collections::HashMap;

/// Partition mono items into at most `max_cgus` codegen units.
///
/// Returns a vector of CGUs, each containing the indices of the items in that CGU.
pub fn partition(items: &[MonoItemData], max_cgus: usize) -> Vec<Vec<usize>> {
    if items.is_empty() {
        return vec![];
    }

    // Group items by source module.
    let mut module_groups: HashMap<u32, Vec<usize>> = HashMap::new();
    for (idx, item) in items.iter().enumerate() {
        module_groups
            .entry(item.source_module)
            .or_default()
            .push(idx);
    }

    let mut groups: Vec<Vec<usize>> = module_groups.into_values().collect();

    // If we have more groups than allowed, merge the smallest groups into the largest.
    while groups.len() > max_cgus {
        // Find index of smallest group and largest group by length.
        let (smallest_idx, _) = groups
            .iter()
            .enumerate()
            .min_by_key(|(_, g)| g.len())
            .expect("groups is non-empty");

        let (largest_idx, _) = groups
            .iter()
            .enumerate()
            .max_by_key(|(_, g)| g.len())
            .expect("groups is non-empty");

        if smallest_idx == largest_idx {
            // All groups equal size; just merge any two.
            // Take first and second, merge second into first.
            if groups.len() < 2 {
                break;
            }
            let second = groups.remove(1);
            groups[0].extend(second);
        } else {
            // Move elements from smallest group into largest group.
            let mut smallest = groups.remove(smallest_idx);
            // After removal, largest_idx may have shifted if it was after smallest.
            let target = if largest_idx > smallest_idx {
                largest_idx - 1
            } else {
                largest_idx
            };
            groups[target].append(&mut smallest);
        }
    }

    groups
}
