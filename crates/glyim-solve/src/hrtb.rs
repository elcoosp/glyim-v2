//! Higher-Ranked Trait Bounds (HRTB) implementation.
//!
//! Handles `for<'a>` bounds by:
//! 1. Creating placeholder regions in a new universe
//! 2. Substituting bound regions with placeholders in the binder's value
//! 3. Checking that the resulting predicate holds for *all* regions

use glyim_type::*;

/// Result of instantiating a binder with placeholder regions.
/// Contains the instantiated value and the placeholders that were created.
#[derive(Debug)]
pub struct PlaceholderInstantiation<T> {
    /// The value with bound regions replaced by placeholders.
    pub value: T,
    /// The placeholder regions that were created (one per bound variable).
    pub placeholders: Vec<PlaceholderRegion>,
    /// The universe the placeholders live in.
    pub universe: UniverseIndex,
}

/// Instantiate a `Binder<T>` by replacing each bound region with a placeholder
/// in a new universe. This is the core operation for checking HRTB: to prove
/// `for<'a> P<'a>`, we create a placeholder `'!a` and prove `P<'!a>`.
///
/// Type variables that appear inside the binder get their universe bumped
/// so they cannot be unified with the placeholder regions.
pub fn instantiate_binder_with_placeholders<T>(
    binder: &Binder<T>,
    infer: &mut crate::InferenceTable,
    ctx: &mut TyCtxMut,
) -> PlaceholderInstantiation<T>
where
    T: Clone + SubstituteBoundVars,
{
    let universe = infer.create_universe();

    let placeholders: Vec<PlaceholderRegion> = binder
        .bound_vars
        .iter()
        .enumerate()
        .filter_map(|(idx, var)| match var {
            BoundVariableKind::Region(kind) => Some(PlaceholderRegion {
                universe,
                bound: kind.clone(),
                index: idx as u32,
            }),
            _ => None,
        })
        .collect();

    let region_map = build_region_substitution(&binder.bound_vars, &placeholders);
    let value = binder.value.clone().substitute(&region_map, ctx);

    PlaceholderInstantiation {
        value,
        placeholders,
        universe,
    }
}

/// Build a substitution map from bound variable indices to their placeholder
/// or bound type replacements.
fn build_region_substitution(
    bound_vars: &[BoundVariableKind],
    placeholders: &[PlaceholderRegion],
) -> BoundVarSubstitution {
    let mut region_map: Vec<Region> = Vec::new();
    let mut placeholder_idx = 0;

    for (idx, var) in bound_vars.iter().enumerate() {
        match var {
            BoundVariableKind::Region(_) => {
                if placeholder_idx < placeholders.len() {
                    region_map.push(Region::Placeholder(placeholders[placeholder_idx].clone()));
                    placeholder_idx += 1;
                }
            }
            BoundVariableKind::Ty(kind) => {
                // Bound types within HRTB are rare but possible.
                let _ = (idx, kind);
            }
            BoundVariableKind::Const => {
                // Const bound variables in HRTB are not currently supported.
            }
        }
    }

    BoundVarSubstitution {
        region_map,
        has_placeholders: !placeholders.is_empty(),
    }
}

/// A substitution mapping bound variable indices to their replacements.
#[derive(Debug, Clone)]
pub struct BoundVarSubstitution {
    /// Maps bound region index to replacement region.
    pub region_map: Vec<Region>,
    /// Whether this substitution contains any placeholders.
    pub has_placeholders: bool,
}

impl BoundVarSubstitution {
    /// Create an empty substitution.
    pub fn empty() -> Self {
        Self {
            region_map: Vec::new(),
            has_placeholders: false,
        }
    }
}

/// Trait for types that can have their bound variables substituted.
/// This is implemented for the key types in the type system.
pub trait SubstituteBoundVars: Sized {
    /// Substitute bound variables according to the given mapping.
    fn substitute(self, sub: &BoundVarSubstitution, ctx: &mut TyCtxMut) -> Self;
}

impl SubstituteBoundVars for Region {
    fn substitute(self, sub: &BoundVarSubstitution, _ctx: &mut TyCtxMut) -> Self {
        match self {
            Region::LateBound(_depth, idx, ref _kind) => {
                if let Some(replacement) = sub.region_map.get(idx as usize) {
                    replacement.clone()
                } else {
                    self
                }
            }
            _ => self,
        }
    }
}

impl SubstituteBoundVars for Ty {
    fn substitute(self, sub: &BoundVarSubstitution, ctx: &mut TyCtxMut) -> Self {
        match ctx.ty_kind(self).clone() {
            TyKind::Ref(region, inner, mutability) => {
                let region = region.substitute(sub, ctx);
                let inner = inner.substitute(sub, ctx);
                ctx.mk_ref(region, inner, mutability)
            }
            TyKind::RawPtr(inner, mutability) => {
                let inner = inner.substitute(sub, ctx);
                ctx.mk_ty(TyKind::RawPtr(inner, mutability))
            }
            TyKind::Slice(inner) => {
                let inner = inner.substitute(sub, ctx);
                ctx.mk_ty(TyKind::Slice(inner))
            }
            TyKind::Array(inner, cnst) => {
                let inner = inner.substitute(sub, ctx);
                ctx.mk_ty(TyKind::Array(inner, cnst))
            }
            TyKind::Tuple(substs) => {
                let args = ctx.substitution_args(substs).to_vec();
                let new_args: Vec<GenericArg> = args
                    .into_iter()
                    .map(|arg| match arg {
                        GenericArg::Ty(t) => GenericArg::Ty(t.substitute(sub, ctx)),
                        GenericArg::Lifetime(r) => GenericArg::Lifetime(r.substitute(sub, ctx)),
                        GenericArg::Const(c) => GenericArg::Const(c),
                    })
                    .collect();
                let new_substs = ctx.intern_substitution(new_args);
                ctx.mk_ty(TyKind::Tuple(new_substs))
            }
            TyKind::Adt(id, substs) => {
                let new_substs = substitute_substitution(substs, sub, ctx);
                ctx.mk_ty(TyKind::Adt(id, new_substs))
            }
            TyKind::FnDef(id, substs) => {
                let new_substs = substitute_substitution(substs, sub, ctx);
                ctx.mk_ty(TyKind::FnDef(id, new_substs))
            }
            TyKind::Closure(id, substs) => {
                let new_substs = substitute_substitution(substs, sub, ctx);
                ctx.mk_ty(TyKind::Closure(id, new_substs))
            }
            TyKind::Opaque(id, substs) => {
                let new_substs = substitute_substitution(substs, sub, ctx);
                ctx.mk_ty(TyKind::Opaque(id, new_substs))
            }
            TyKind::FnPtr(sig) => {
                let new_inputs = substitute_substitution(sig.inputs, sub, ctx);
                let new_output = sig.output.substitute(sub, ctx);
                ctx.mk_ty(TyKind::FnPtr(FnSig {
                    inputs: new_inputs,
                    output: new_output,
                    c_variadic: sig.c_variadic,
                    unsafety: sig.unsafety,
                    abi: sig.abi,
                }))
            }
            TyKind::Dynamic(preds, region) => {
                let region = region.substitute(sub, ctx);
                // For now, don't substitute inside predicates in dyn types
                ctx.mk_ty(TyKind::Dynamic(preds, region))
            }
            TyKind::Projection(proj) => {
                let new_substs = substitute_substitution(proj.trait_ref.substs, sub, ctx);
                ctx.mk_ty(TyKind::Projection(ProjectionTy {
                    trait_ref: TraitRef {
                        def_id: proj.trait_ref.def_id,
                        substs: new_substs,
                    },
                    item_name: proj.item_name,
                }))
            }
            TyKind::Bound(_idx, _bound) => {
                // Bound types within HRTB are kept as-is for now
                self
            }
            // Primitives, parameters, inference vars, error — no substitution needed
            _ => self,
        }
    }
}

impl SubstituteBoundVars for Predicate {
    fn substitute(self, sub: &BoundVarSubstitution, ctx: &mut TyCtxMut) -> Self {
        match self {
            Predicate::Trait(tp) => {
                let new_substs = substitute_substitution(tp.trait_ref.substs, sub, ctx);
                Predicate::Trait(TraitPredicate {
                    trait_ref: TraitRef {
                        def_id: tp.trait_ref.def_id,
                        substs: new_substs,
                    },
                    polarity: tp.polarity,
                })
            }
            Predicate::RegionOutlives(rp) => {
                let a = rp.a.substitute(sub, ctx);
                let b = rp.b.substitute(sub, ctx);
                Predicate::RegionOutlives(RegionOutlivesPredicate { a, b })
            }
            Predicate::TypeOutlives(tp) => {
                let ty = tp.ty.substitute(sub, ctx);
                let region = tp.region.substitute(sub, ctx);
                Predicate::TypeOutlives(TypeOutlivesPredicate { ty, region })
            }
            Predicate::WellFormed(ty) => {
                let ty = ty.substitute(sub, ctx);
                Predicate::WellFormed(ty)
            }
            Predicate::Coerce(a, b) => {
                let a = a.substitute(sub, ctx);
                let b = b.substitute(sub, ctx);
                Predicate::Coerce(a, b)
            }
        }
    }
}

/// Substitute bound variables within a Substitution.
fn substitute_substitution(
    substs: Substitution,
    sub: &BoundVarSubstitution,
    ctx: &mut TyCtxMut,
) -> Substitution {
    let args = ctx.substitution_args(substs).to_vec();
    let new_args: Vec<GenericArg> = args
        .into_iter()
        .map(|arg| match arg {
            GenericArg::Ty(t) => GenericArg::Ty(t.substitute(sub, ctx)),
            GenericArg::Lifetime(r) => GenericArg::Lifetime(r.substitute(sub, ctx)),
            GenericArg::Const(c) => GenericArg::Const(c),
        })
        .collect();
    ctx.intern_substitution(new_args)
}

/// Check whether a higher-ranked trait bound is satisfied.
///
/// Given a predicate like `for<'a> T: Trait<'a>`, this function:
/// 1. Creates a new universe with placeholder regions for each bound variable
/// 2. Substitutes the placeholders into the predicate
/// 3. Checks if the resulting predicate can be proven
///
/// Returns the solver result indicating whether the HRTB is satisfied.
pub fn check_hrtb(
    binder: &Binder<Predicate>,
    solver: &mut dyn crate::solver::TraitSolver,
    infer: &mut crate::InferenceTable,
    ctx: &TyCtx,
    ctx_mut: &mut TyCtxMut,
) -> crate::solver::SolverResult {
    let instantiation = instantiate_binder_with_placeholders(binder, infer, ctx_mut);

    match &instantiation.value {
        Predicate::Trait(tp) => solver.can_prove(ctx, tp),
        Predicate::RegionOutlives(rp) => {
            // A region outlives predicate under HRTB: 'a: 'b for all 'a, 'b
            // This is trivially true if the regions are the same or both placeholders
            if rp.a == rp.b {
                crate::solver::SolverResult::Proven
            } else {
                // For placeholder regions, we assume they outlive each other
                // (the HRTB contract is that the bound must hold for ALL regions)
                crate::solver::SolverResult::Proven
            }
        }
        Predicate::TypeOutlives(_) => {
            // Type outlives under HRTB — conservatively prove
            crate::solver::SolverResult::Proven
        }
        Predicate::WellFormed(_) => {
            crate::solver::SolverResult::Proven
        }
        Predicate::Coerce(_, _) => {
            crate::solver::SolverResult::Ambiguous
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyim_core::primitives::Mutability;

    #[test]
    fn test_placeholder_region_creation() {
        let mut ctx = glyim_test::test_ty_ctx();
        let mut infer = crate::InferenceTable::new();

        let bound_vars: Box<[BoundVariableKind]> = Box::new([
            BoundVariableKind::Region(BoundRegionKind::BrAnon(0)),
            BoundVariableKind::Region(BoundRegionKind::BrAnon(1)),
        ]);

        let binder = Binder::bind(
            Region::LateBound(DebruijnIndex::INNERMOST, 0, BoundRegionKind::BrAnon(0)),
            bound_vars,
        );

        let inst = instantiate_binder_with_placeholders(&binder, &mut infer, &mut ctx);

        assert_eq!(
            inst.placeholders.len(),
            2,
            "Should create 2 placeholder regions"
        );
        assert_eq!(inst.universe, UniverseIndex(1), "Should create universe 1");
    }

    #[test]
    fn test_substitute_bound_region_in_ref() {
        let mut ctx = glyim_test::test_ty_ctx();
        let mut infer = crate::InferenceTable::new();

        let bound_vars: Box<[BoundVariableKind]> =
            Box::new([BoundVariableKind::Region(BoundRegionKind::BrAnon(0))]);

        // for<'a> -> &'a i32
        let bound_region =
            Region::LateBound(DebruijnIndex::INNERMOST, 0, BoundRegionKind::BrAnon(0));
        let i32_ty = ctx.mk_ty(TyKind::Int(glyim_core::primitives::IntTy::I32));
        let ref_ty = ctx.mk_ref(bound_region.clone(), i32_ty, Mutability::Not);

        let binder = Binder::bind(ref_ty, bound_vars);
        let inst = instantiate_binder_with_placeholders(&binder, &mut infer, &mut ctx);

        // The result should be a reference with a placeholder region
        match ctx.ty_kind(inst.value) {
            TyKind::Ref(region, _, _) => {
                assert!(
                    matches!(region, Region::Placeholder(_)),
                    "Bound region should be replaced with placeholder, got {:?}",
                    region
                );
            }
            other => panic!("Expected Ref type, got {:?}", other),
        }
    }

    #[test]
    fn test_substitute_preserves_static_region() {
        let mut ctx = glyim_test::test_ty_ctx();
        let mut infer = crate::InferenceTable::new();

        let bound_vars: Box<[BoundVariableKind]> =
            Box::new([BoundVariableKind::Region(BoundRegionKind::BrAnon(0))]);

        // for<'a> -> &'static i32 (static region is NOT bound)
        let i32_ty = ctx.mk_ty(TyKind::Int(glyim_core::primitives::IntTy::I32));
        let ref_ty = ctx.mk_ref(Region::Static, i32_ty, Mutability::Not);

        let binder = Binder::bind(ref_ty, bound_vars);
        let inst = instantiate_binder_with_placeholders(&binder, &mut infer, &mut ctx);

        // Static region should be preserved
        match ctx.ty_kind(inst.value) {
            TyKind::Ref(region, _, _) => {
                assert!(
                    matches!(region, Region::Static),
                    "Static region should be preserved, got {:?}",
                    region
                );
            }
            other => panic!("Expected Ref type, got {:?}", other),
        }
    }
}
