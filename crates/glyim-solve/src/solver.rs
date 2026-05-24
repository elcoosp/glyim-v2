use glyim_core::def_id::{ImplDefId, TraitDefId};
use glyim_core::interner::Name;
use glyim_type::*;

/// Information returned by the trait solver for `Iterator::next`.
#[derive(Clone, Debug)]
pub struct SolverIteratorNextInfo {
    pub fn_def_id: glyim_core::def_id::FnDefId,
    pub fn_substs: glyim_type::Substitution,
    pub fn_ty: glyim_type::Ty,
    pub option_ty: glyim_type::Ty,
    pub discr_ty: glyim_type::Ty,
    pub ref_iter_ty: glyim_type::Ty,
}

pub trait TraitSolver {
    fn can_prove(&mut self, ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult;
    fn evaluate_predicate(&mut self, ctx: &TyCtx, predicate: &Predicate) -> SolverResult;
    /// Get the `Iterator::next` method info for a given iterator type.
    fn iterator_next_info(
        &self,
        ctx_mut: &mut glyim_type::TyCtxMut,
        iter_ty: glyim_type::Ty,
        elem_ty: glyim_type::Ty,
    ) -> Option<SolverIteratorNextInfo>;
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
    pub(crate) builtin_next_fn_id: Option<glyim_core::def_id::FnDefId>,
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
            builtin_next_fn_id: None,
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
    fn substs_has_local_type(&self, substs: &Substitution) -> bool {
        !substs.is_empty()
    }

    /// Naive overlap check between two trait references.
    fn impls_overlap(&self, a: &TraitRef, b: &TraitRef) -> bool {
        if a.def_id != b.def_id {
            return false;
        }

        // Naive check: identical substitutions overlap.
        // A full check would require deep unification with TyCtx, which is not available here.
        a.substs == b.substs
    }
    /// Set the FnDefId of the `Iterator::next` method for built-in support.
    pub fn set_builtin_iterator_next(&mut self, fn_def_id: glyim_core::def_id::FnDefId) {
        self.builtin_next_fn_id = Some(fn_def_id);
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
            (GenericArg::Lifetime(_), GenericArg::Lifetime(_)) => true,
            (GenericArg::Const(pred_const), GenericArg::Const(impl_const)) => {
                pred_const == impl_const
            }
            _ => false,
        }
    }

    fn tys_match(&self, ctx: &TyCtx, pred_ty: Ty, impl_ty: Ty) -> bool {
        if pred_ty == impl_ty {
            return true;
        }

        let pred_kind = ctx.ty_kind(pred_ty);
        let impl_kind = ctx.ty_kind(impl_ty);

        if matches!(impl_kind, TyKind::Param(_)) {
            return true;
        }
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
            (TyKind::Ref(_, pred_ty, pred_mut), TyKind::Ref(_, impl_ty, impl_mut)) => {
                if pred_mut != impl_mut {
                    return false;
                }
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

    fn prove_trait(&mut self, ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult {
        if predicate.polarity == ImplPolarity::Negative {
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
                proven_count += 1;
                continue;
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
            // Multiple impls fully apply -> Ambiguous
            SolverResult::Ambiguous
        } else if proven_count == 1 {
            SolverResult::Proven
        } else if ambiguous_count > 0 {
            SolverResult::Ambiguous
        } else {
            SolverResult::DefiniteNo
        }
    }

    fn try_resolve_projection(&mut self, ctx: &TyCtx, ty: Ty) -> SolverResult {
        let kind = ctx.ty_kind(ty);
        if let TyKind::Projection(proj) = kind {
            let trait_pred = TraitPredicate {
                trait_ref: proj.trait_ref.clone(),
                polarity: ImplPolarity::Positive,
            };
            match self.prove_trait(ctx, &trait_pred) {
                SolverResult::Proven => SolverResult::Proven,
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
        (TyKind::Ref(_, inner_a, mut_a), TyKind::RawPtr(inner_b, mut_b)) => {
            // &T -> *const T (Not -> Not) and &mut T -> *mut T (Mut -> Mut)
            if *mut_a != *mut_b {
                return false;
            }
            can_coerce(ctx, *inner_a, *inner_b)
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
            Predicate::WellFormed(ty) => self.try_resolve_projection(ctx, *ty),
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
    fn iterator_next_info(
        &self,
        ctx_mut: &mut glyim_type::TyCtxMut,
        iter_ty: glyim_type::Ty,
        elem_ty: glyim_type::Ty,
    ) -> Option<SolverIteratorNextInfo> {
        let next_def_id = self.trait_ctx.builtin_next_fn_id?;
        // Build substitution: Self -> iter_ty (Iterator has one type parameter)
        let substs = ctx_mut.intern_substitution(vec![glyim_type::GenericArg::Ty(iter_ty)]);
        let fn_ty = ctx_mut.mk_ty(glyim_type::TyKind::FnDef(next_def_id, substs));
        // Option<elem_ty> - placeholder ADT ID (should be lang item)
        let option_adt = glyim_core::def_id::AdtId::from_raw(101);
        let opt_subst = ctx_mut.intern_substitution(vec![glyim_type::GenericArg::Ty(elem_ty)]);
        let option_ty = ctx_mut.mk_ty(glyim_type::TyKind::Adt(option_adt, opt_subst));
        let discr_ty = ctx_mut.mk_ty(glyim_type::TyKind::Uint(glyim_core::primitives::UintTy::U8));
        let ref_iter_ty = ctx_mut.mk_ref(
            glyim_type::Region::Erased,
            iter_ty,
            glyim_core::primitives::Mutability::Mut,
        );
        Some(SolverIteratorNextInfo {
            fn_def_id: next_def_id,
            fn_substs: substs,
            fn_ty,
            option_ty,
            discr_ty,
            ref_iter_ty,
        })
    }
}
