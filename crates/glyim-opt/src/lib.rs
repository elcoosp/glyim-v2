//! MIR optimizations (constant propagation, dead code elimination, etc.)

use glyim_mir::Body;
use glyim_type::TyCtx;
use std::sync::Arc;

mod constant_prop;
mod dce;
mod cfg_simplify;
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
