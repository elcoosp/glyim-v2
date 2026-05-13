use glyim_type::*;
use glyim_core::def_id::{TraitDefId, ImplDefId};
use glyim_core::interner::Name;

pub trait TraitSolver {
    fn can_prove(&mut self, ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult;
    fn evaluate_predicate(&mut self, ctx: &TyCtx, predicate: &Predicate) -> SolverResult;
}

#[derive(Clone, Debug)]
pub enum SolverResult {
    Proven,
    Ambiguous,
    DefiniteNo,
}

pub struct TraitContext {
    trait_defs: Vec<TraitDef>,
    impl_defs: Vec<ImplDef>,
}

#[derive(Clone, Debug)]
pub struct TraitDef {
    pub def_id: TraitDefId,
    pub name: Name,
    pub associated_types: Vec<Name>,
    pub predicates: Vec<Predicate>,
}

#[derive(Clone, Debug)]
pub struct ImplDef {
    pub def_id: ImplDefId,
    pub trait_ref: TraitRef,
    pub predicates: Vec<Predicate>,
}

impl TraitContext {
    pub fn new() -> Self { Self { trait_defs: Vec::new(), impl_defs: Vec::new() } }
    pub fn register_trait(&mut self, def: TraitDef) { self.trait_defs.push(def); }
    pub fn register_impl(&mut self, def: ImplDef) { self.impl_defs.push(def); }
    pub fn impls_of_trait(&self, trait_id: TraitDefId) -> impl Iterator<Item = &ImplDef> {
        self.impl_defs.iter().filter(move |i| i.trait_ref.def_id == trait_id)
    }
}

impl Default for TraitContext { fn default() -> Self { Self::new() } }

pub struct SimpleTraitSolver<'a> {
    trait_ctx: &'a TraitContext,
}

impl<'a> SimpleTraitSolver<'a> {
    pub fn new(trait_ctx: &'a TraitContext) -> Self { Self { trait_ctx } }
}

impl TraitSolver for SimpleTraitSolver<'_> {
    fn can_prove(&mut self, _ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult {
        let has_impl = self.trait_ctx.impls_of_trait(predicate.trait_ref.def_id).any(|_| true);
        if has_impl { SolverResult::Proven } else { SolverResult::Ambiguous }
    }

    fn evaluate_predicate(&mut self, ctx: &TyCtx, predicate: &Predicate) -> SolverResult {
        match predicate {
            Predicate::Trait(trait_pred) => self.can_prove(ctx, trait_pred),
            Predicate::WellFormed(_) | Predicate::TypeOutlives(_) | Predicate::RegionOutlives(_) => SolverResult::Proven,
            Predicate::Coerce(_, _) => SolverResult::Ambiguous,
        }
    }
}
