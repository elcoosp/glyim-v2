//! MIR optimizations (constant propagation, dead code elimination, etc.)
#![allow(missing_docs)]

use glyim_mir::Body;
use glyim_type::TyCtx;
use std::sync::Arc;

mod cfg_simplify;
mod constant_prop;
mod dce;
mod slice_desugar;
mod unreachable_elim;

#[derive(Clone, Debug)]
pub struct Optimized {
    pub body: Body,
}

pub fn optimize(ctx: &TyCtx, body: &Arc<Body>) -> Optimized {
    let mut body = (**body).clone();
    // Runs first and unconditionally: every later pass, and codegen after
    // them, assumes `ConstantIndex`/`Subslice` projections are always
    // terminal (see slice_desugar's module doc). This is a no-op for the
    // (overwhelming majority of) bodies that don't contain any such
    // projection.
    slice_desugar::run(ctx, &mut body);
    constant_prop::run(ctx, &mut body);
    dce::run(ctx, &mut body);
    cfg_simplify::run(ctx, &mut body);
    unreachable_elim::run(ctx, &mut body);
    Optimized { body }
}

#[cfg(test)]
mod tests;

/// Stub for drop elaboration. Will be implemented later.
mod drop_elaboration;

/// Run drop elaboration on the MIR body.
pub fn elaborate_drops(ctx: &TyCtx, body: &mut Body) {
    drop_elaboration::run(ctx, body);
}
