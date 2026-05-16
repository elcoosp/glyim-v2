//! Mono item caching for the pipeline.
//!
//! Provides a thin wrapper around `MonoCtx` that integrates with
//! `Database`'s mono cache for cross-compilation reuse.

use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_lower::mono::MonoItemData;
use glyim_mir::Body;
use glyim_type::Substitution;
use std::sync::Arc;

/// Pipeline-level mono cache that wraps MonoCtx and tracks
/// which items have been collected for potential reuse.
pub(crate) struct PipelineMonoCache {
    symbols: Vec<String>,
}

impl PipelineMonoCache {
    /// Create a cache from the collected mono items.
    pub(crate) fn from_items(items: &[MonoItemData]) -> Self {
        let symbols = items.iter().map(|d| d.symbol.clone()).collect();
        PipelineMonoCache { symbols }
    }

    /// Get the cached item symbols.
    pub(crate) fn symbols(&self) -> &[String] {
        &self.symbols
    }

    /// Get the number of cached items.
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Check if the cache is empty.
    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }
}

/// Build a MIR body provider that looks up pre-lowered bodies by DefId.
/// If a body is not found, returns a dummy body.
pub(crate) fn make_mir_body_provider(
    bodies: &std::collections::HashMap<DefId, Arc<Body>>,
) -> impl Fn(DefId, &Substitution) -> Arc<Body> + '_ {
    move |def_id: DefId, substs: &Substitution| -> Arc<Body> {
        if let Some(body) = bodies.get(&def_id) {
            if substs.is_empty() {
                body.clone()
            } else {
                // Generic instantiation: for v0.1.0, return the un-instantiated
                // body and emit a stub warning. Full instantiation requires
                // MonoCtx::instantiate which needs TyCtx.
                tracing::warn!(
                    "STUB: generic instantiation not yet fully implemented for {:?}",
                    def_id
                );
                body.clone()
            }
        } else {
            tracing::warn!("STUB: MIR body not found for def_id {:?}", def_id);
            Arc::new(Body::dummy(DefId::new(
                CrateId::from_raw(0),
                LocalDefId::from_raw(0),
            )))
        }
    }
}

/// Build a drop glue body provider.
/// For v0.1.0, returns a dummy body for any type.
pub(crate) fn make_drop_glue_provider() -> impl Fn(glyim_type::Ty) -> Arc<Body> {
    move |ty: glyim_type::Ty| -> Arc<Body> {
        tracing::warn!("STUB: drop glue generation for type {:?}", ty);
        Arc::new(Body::dummy(DefId::new(
            CrateId::from_raw(0),
            LocalDefId::from_raw(0),
        )))
    }
}

/// Compute the maximum number of codegen units based on available parallelism.
pub(crate) fn compute_max_cgus() -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    cores.min(16).max(1)
}
