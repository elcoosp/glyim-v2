use glyim_type::TyCtx;
use crate::solver::TraitSolver;

pub struct FulfillmentCtx<'a> {
    pub solver: &'a mut dyn TraitSolver,
}

impl<'a> FulfillmentCtx<'a> {
    pub fn new(_ctx: &'a TyCtx, solver: &'a mut dyn TraitSolver) -> Self {
        Self { solver }
    }
}
