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

    /// Checks coherence (orphan rules and overlap detection).
    /// Returns an error string if the impl violates coherence.
    pub fn check_coherence(&self, impl_def: &ImplDef) -> Result<(), String> {
        // 1. Orphan rule: At least one of the type arguments in the trait reference
        //    must be a local type defined in the current crate.
        let has_local_type = self.substs_has_local_type(&impl_def.trait_ref.substs);
        if !has_local_type {
            return Err(format!(
                "Orphan rule violation: impl for trait {:?} has no local types",
                impl_def.trait_ref.def_id
            ));
        }

        // 2. Overlap detection: Ensure no two impls for the same trait overlap.
        for other in self.impls_of_trait(impl_def.trait_ref.def_id) {
            if other.def_id == impl_def.def_id {
                continue;
            }
            if self.impls_overlap(&impl_def.trait_ref, &other.trait_ref) {
                return Err(format!(
                    "Overlap violation: impl {:?} overlaps with impl {:?}",
                    impl_def.def_id, other.def_id
                ));
            }
        }
        Ok(())
    }

    /// Naive check to see if substitutions contain a local type.
    /// Assumes types from the current crate have a matching CrateId (typically 0 for local).
    fn substs_has_local_type(&self, substs: &Substitution) -> bool {
        // A real implementation would look up the AdtId to check if it belongs to the local crate.
        // Here we assume that if there are any type arguments, one is local enough to pass the orphan rule.
        // A fully rigorous check needs CrateId access from TyCtx.
        !substs.is_empty()
    }

    /// Naive overlap check between two trait references.
    /// Two impls overlap if their substitutions can unify.
    fn impls_overlap(&self, a: &TraitRef, b: &TraitRef) -> bool {
        if a.def_id != b.def_id {
            return false;
        }

        let a_args = a.substs;
        let b_args = b.substs;

        // If both have exactly 0 substs, they trivially overlap
        if a_args.is_empty() && b_args.is_empty() {
            return true;
        }

        // We can't do deep type unification without TyCtx here.
        // A rigorous check requires attempting to unify the Substitutions.
        // As a heuristic/placeholder, if lengths match we consider them potentially overlapping.
        // A full implementation would use InferenceTable::unify here.
        a_args.len() == b_args.len()
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

    /// Attempt to match the predicate's TraitRef against an impl's TraitRef.
    /// Returns true if the trait def_id matches and the substitutions unify.
    fn matches_trait_ref(
        &self,
        ctx: &TyCtx,
        predicate_ref: &TraitRef,
        impl_ref: &TraitRef,
    ) -> bool {
        if predicate_ref.def_id != impl_ref.def_id {
            return false;
        }

        let pred_args = ctx.substitution_args(predicate_ref.substs);
        let impl_args = ctx.substitution_args(impl_ref.substs);

        if pred_args.len() != impl_args.len() {
            return false;
        }

        for (pred_arg, impl_arg) in pred_args.iter().zip(impl_args.iter()) {
            if !self.generic_args_match(ctx, pred_arg, impl_arg) {
                return false;
            }
        }

        true
    }

    /// Recursively checks if a predicate's generic argument can match an impl's generic argument.
    fn generic_args_match(
        &self,
        ctx: &TyCtx,
        pred_arg: &GenericArg,
        impl_arg: &GenericArg,
    ) -> bool {
        match (pred_arg, impl_arg) {
            (GenericArg::Ty(pred_ty), GenericArg::Ty(impl_ty)) => {
                self.tys_match(ctx, *pred_ty, *impl_ty)
            }
            (GenericArg::Lifetime(_), GenericArg::Lifetime(_)) => {
                // Lifetimes are typically erased or handled by region outlives,
                // so we treat them as matching for trait resolution.
                true
            }
            (GenericArg::Const(pred_const), GenericArg::Const(impl_const)) => {
                // Constants must match exactly unless one is a generic param.
                // For now, just check deep equality.
                pred_const == impl_const
            }
            _ => false,
        }
    }

    /// Checks if types match. Allows impl type parameters to unify with concrete types.
    fn tys_match(&self, ctx: &TyCtx, pred_ty: Ty, impl_ty: Ty) -> bool {
        if pred_ty == impl_ty {
            return true;
        }

        let pred_kind = ctx.ty_kind(pred_ty);
        let impl_kind = ctx.ty_kind(impl_ty);

        // If the impl has a type parameter, it can match any concrete predicate type.
        if matches!(impl_kind, TyKind::Param(_)) {
            return true;
        }
        // If the predicate has an infer variable, it can unify with anything.
        if matches!(pred_kind, TyKind::Infer(_)) {
            return true;
        }

        match (pred_kind, impl_kind) {
            (TyKind::Adt(pred_id, pred_substs), TyKind::Adt(impl_id, impl_substs)) => {
                if pred_id != impl_id {
                    return false;
                }
                let pred_args = ctx.substitution_args(*pred_substs);
                let impl_args = ctx.substitution_args(*impl_substs);
                if pred_args.len() != impl_args.len() {
                    return false;
                }
                pred_args
                    .iter()
                    .zip(impl_args.iter())
                    .all(|(p, i)| self.generic_args_match(ctx, p, i))
            }
            (TyKind::Ref(pred_r, pred_ty, pred_mut), TyKind::Ref(impl_r, impl_ty, impl_mut)) => {
                if pred_mut != impl_mut {
                    return false;
                }
                // Regions are mostly erased in simple trait matching
                let _ = (pred_r, impl_r);
                self.tys_match(ctx, *pred_ty, *impl_ty)
            }
            (TyKind::RawPtr(pred_ty, pred_mut), TyKind::RawPtr(impl_ty, impl_mut)) => {
                if pred_mut != impl_mut {
                    return false;
                }
                self.tys_match(ctx, *pred_ty, *impl_ty)
            }
            (TyKind::Slice(pred_ty), TyKind::Slice(impl_ty)) => {
                self.tys_match(ctx, *pred_ty, *impl_ty)
            }
            (TyKind::Array(pred_ty, _), TyKind::Array(impl_ty, _)) => {
                self.tys_match(ctx, *pred_ty, *impl_ty)
            }
            (TyKind::Tuple(pred_substs), TyKind::Tuple(impl_substs)) => {
                let pred_args = ctx.substitution_args(*pred_substs);
                let impl_args = ctx.substitution_args(*impl_substs);
                if pred_args.len() != impl_args.len() {
                    return false;
                }
                pred_args
                    .iter()
                    .zip(impl_args.iter())
                    .all(|(p, i)| self.generic_args_match(ctx, p, i))
            }
            (TyKind::Never, TyKind::Never)
            | (TyKind::Unit, TyKind::Unit)
            | (TyKind::Bool, TyKind::Bool) => true,
            (TyKind::Int(p), TyKind::Int(i)) => p == i,
            (TyKind::Uint(p), TyKind::Uint(i)) => p == i,
            (TyKind::Float(p), TyKind::Float(i)) => p == i,
            (TyKind::FnPtr(p_sig), TyKind::FnPtr(i_sig)) => {
                // Function pointer matching (simplified)
                if p_sig.c_variadic != i_sig.c_variadic
                    || p_sig.unsafety != i_sig.unsafety
                    || p_sig.abi != i_sig.abi
                {
                    return false;
                }
                if !self.tys_match(ctx, p_sig.output, i_sig.output) {
                    return false;
                }
                let p_inputs = ctx.substitution_args(p_sig.inputs);
                let i_inputs = ctx.substitution_args(i_sig.inputs);
                if p_inputs.len() != i_inputs.len() {
                    return false;
                }
                p_inputs
                    .iter()
                    .zip(i_inputs.iter())
                    .all(|(p, i)| self.generic_args_match(ctx, p, i))
            }
            _ => false,
        }
    }

    /// Try to prove a trait predicate by finding a matching impl and evaluating its where clauses.
    fn prove_trait(&mut self, ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult {
        if predicate.polarity == ImplPolarity::Negative {
            // If there is a positive impl, negative is DefiniteNo.
            // Otherwise, we assume it's true (Ambiguous for safety).
            let has_positive = self
                .trait_ctx
                .impls_of_trait(predicate.trait_ref.def_id)
                .any(|impl_def| {
                    self.matches_trait_ref(ctx, &predicate.trait_ref, &impl_def.trait_ref)
                });
            return if has_positive {
                SolverResult::DefiniteNo
            } else {
                SolverResult::Ambiguous
            };
        }

        let mut matching_impls = Vec::new();

        for impl_def in self.trait_ctx.impls_of_trait(predicate.trait_ref.def_id) {
            if self.matches_trait_ref(ctx, &predicate.trait_ref, &impl_def.trait_ref) {
                matching_impls.push(impl_def);
            }
        }

        if matching_impls.is_empty() {
            return SolverResult::DefiniteNo;
        }

        let mut proven_count = 0;
        let mut ambiguous_count = 0;

        for impl_def in matching_impls {
            if impl_def.predicates.is_empty() {
                // No where clauses means it is unconditionally proven
                return SolverResult::Proven;
            }

            let mut all_proven = true;
            let mut any_ambiguous = false;

            for pred in &impl_def.predicates {
                match self.evaluate_predicate(ctx, pred) {
                    SolverResult::Proven => {}
                    SolverResult::Ambiguous => {
                        any_ambiguous = true;
                    }
                    SolverResult::DefiniteNo => {
                        all_proven = false;
                        break;
                    }
                }
            }

            if all_proven {
                if any_ambiguous {
                    ambiguous_count += 1;
                } else {
                    proven_count += 1;
                }
            }
        }

        if proven_count > 1 {
            // Overlapping impls that both fully apply -> Ambiguous
            SolverResult::Ambiguous
        } else if proven_count == 1 {
            SolverResult::Proven
        } else if ambiguous_count > 0 {
            SolverResult::Ambiguous
        } else {
            SolverResult::DefiniteNo
        }
    }

    /// Handles associated type projection.
    /// If a predicate resolves successfully, we can try to deduce associated types.
    /// In the solver, if the predicate contains an Infer var in the projection position,
    /// we consider it proven and let external unification resolve the variable.
    fn try_resolve_projection(&mut self, ctx: &TyCtx, ty: Ty) -> SolverResult {
        let kind = ctx.ty_kind(ty);
        if let TyKind::Projection(proj) = kind {
            // Try to prove the trait portion of the projection
            let trait_pred = TraitPredicate {
                trait_ref: proj.trait_ref.clone(),
                polarity: ImplPolarity::Positive,
            };
            match self.prove_trait(ctx, &trait_pred) {
                SolverResult::Proven => {
                    // If the trait is proven, the projection is well-formed.
                    // Actual type unification for the associated type is handled
                    // by the InferenceTable unifying the Ty containing the Projection.
                    SolverResult::Proven
                }
                other => other,
            }
        } else {
            SolverResult::Proven
        }
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
    fn can_prove(&mut self, ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult {
        self.prove_trait(ctx, predicate)
    }

    fn evaluate_predicate(&mut self, ctx: &TyCtx, predicate: &Predicate) -> SolverResult {
        match predicate {
            Predicate::Trait(trait_pred) => self.can_prove(ctx, trait_pred),
            Predicate::WellFormed(ty) => {
                // Well-formedness often depends on associated types being well-formed.
                self.try_resolve_projection(ctx, *ty)
            }
            Predicate::TypeOutlives(pred) => self.try_resolve_projection(ctx, pred.ty),
            Predicate::RegionOutlives(_) => SolverResult::Proven,
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
