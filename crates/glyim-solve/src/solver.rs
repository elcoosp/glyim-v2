use glyim_core::def_id::{ImplDefId, TraitDefId};
use glyim_core::interner::Name;
use glyim_type::*;

pub trait TraitSolver {
    fn can_prove(&mut self, ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult;
    fn evaluate_predicate(&mut self, ctx: &TyCtx, predicate: &Predicate) -> SolverResult;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SolverResult {
    Proven,
    Ambiguous,
    DefiniteNo,
}

pub struct TraitContext {
    trait_defs: Vec<TraitDef>,
    impl_defs: Vec<ImplDef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraitDef {
    pub def_id: TraitDefId,
    pub name: Name,
    pub associated_types: Vec<Name>,
    pub predicates: Vec<Predicate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImplDef {
    pub def_id: ImplDefId,
    pub trait_ref: TraitRef,
    pub predicates: Vec<Predicate>,
}

impl TraitContext {
    pub fn new() -> Self {
        Self {
            trait_defs: Vec::new(),
            impl_defs: Vec::new(),
        }
    }
    pub fn register_trait(&mut self, def: TraitDef) {
        self.trait_defs.push(def);
    }
    pub fn register_impl(&mut self, def: ImplDef) {
        self.impl_defs.push(def);
    }
    pub fn impls_of_trait(&self, trait_id: TraitDefId) -> impl Iterator<Item = &ImplDef> {
        self.impl_defs
            .iter()
            .filter(move |i| i.trait_ref.def_id == trait_id)
    }
    #[cfg(test)]
    pub(crate) fn trait_defs(&self) -> &[TraitDef] {
        &self.trait_defs
    }
    #[cfg(test)]
    pub(crate) fn impl_defs(&self) -> &[ImplDef] {
        &self.impl_defs
    }
}
impl Default for TraitContext {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SimpleTraitSolver<'a> {
    trait_ctx: &'a TraitContext,
}
impl<'a> SimpleTraitSolver<'a> {
    pub fn new(trait_ctx: &'a TraitContext) -> Self {
        Self { trait_ctx }
    }
}

fn can_coerce(ctx: &TyCtx, a: Ty, b: Ty) -> bool {
    if a == b {
        return true;
    }
    match (ctx.ty_kind(a), ctx.ty_kind(b)) {
        (TyKind::Array(elem_a, _), TyKind::Slice(elem_b)) if elem_a == elem_b => true,
        (TyKind::Ref(_, inner_a, mut_a), TyKind::Ref(_, inner_b, mut_b)) => {
            (mut_a == mut_b)
                || (*mut_a == glyim_core::primitives::Mutability::Mut
                    && *mut_b == glyim_core::primitives::Mutability::Not)
                    && can_coerce(ctx, *inner_a, *inner_b)
        }
        (TyKind::RawPtr(inner_a, mut_a), TyKind::RawPtr(inner_b, mut_b)) => {
            (mut_a == mut_b)
                || (*mut_a == glyim_core::primitives::Mutability::Mut
                    && *mut_b == glyim_core::primitives::Mutability::Not)
                    && can_coerce(ctx, *inner_a, *inner_b)
        }
        _ => false,
    }
}

impl TraitSolver for SimpleTraitSolver<'_> {
    fn can_prove(&mut self, _ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult {
        if predicate.polarity == ImplPolarity::Negative {
            return SolverResult::Ambiguous;
        }
        let has_impl = self
            .trait_ctx
            .impls_of_trait(predicate.trait_ref.def_id)
            .any(|_| true);
        if has_impl {
            SolverResult::Proven
        } else {
            SolverResult::Ambiguous
        }
    }
    fn evaluate_predicate(&mut self, ctx: &TyCtx, predicate: &Predicate) -> SolverResult {
        match predicate {
            Predicate::Trait(trait_pred) => self.can_prove(ctx, trait_pred),
            Predicate::WellFormed(_)
            | Predicate::TypeOutlives(_)
            | Predicate::RegionOutlives(_) => SolverResult::Proven,
            Predicate::Coerce(a, b) => {
                if can_coerce(ctx, *a, *b) {
                    SolverResult::Proven
                } else {
                    SolverResult::Ambiguous
                }
            }
        }
    }
}
