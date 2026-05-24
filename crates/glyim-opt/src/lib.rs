//! MIR optimizations (constant propagation, dead code elimination, etc.)

use glyim_mir::Body;
use glyim_type::TyCtx;
use std::sync::Arc;

mod cfg_simplify;
mod constant_prop;
mod dce;
mod unreachable_elim;

#[derive(Clone, Debug)]
pub struct Optimized {
    pub body: Body,
}

pub fn optimize(ctx: &TyCtx, body: &Arc<Body>) -> Optimized {
    let mut body = (**body).clone();
    constant_prop::run(ctx, &mut body);
    dce::run(ctx, &mut body);
    cfg_simplify::run(ctx, &mut body);
    unreachable_elim::run(ctx, &mut body);
    Optimized { body }
}

#[cfg(test)]
mod tests;

/// Stub for drop elaboration. Will be implemented later.
pub fn elaborate_drops(_ctx: &TyCtx, _body: &mut Body) {
    // TODO: Implement full drop elaboration
    // Use tracing::warn as required by project rules
    let _ = tracing::warn!("STUB: elaborate_drops not yet implemented");
}
