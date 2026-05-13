//! MIR optimizations (constant propagation, dead code elimination, etc.)

use glyim_mir::Body;
use glyim_type::TyCtx;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Optimized {
    pub body: Body,
}

pub fn optimize(ctx: &TyCtx, body: &Arc<Body>) -> Optimized {
    // STUB: for v0.1.0, just clone the body unchanged
    let _ = ctx;
    Optimized {
        body: (**body).clone(),
    }
}
