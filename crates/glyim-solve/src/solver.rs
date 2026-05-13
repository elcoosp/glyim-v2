pub trait TraitSolver {}
pub struct SimpleTraitSolver<'a>(pub &'a TraitContext);
impl<'a> TraitSolver for SimpleTraitSolver<'a> {}
pub struct TraitContext;
