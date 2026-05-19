use glyim_diag::GlyimDiagnostic;
use glyim_type::*;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct Obligation {
    pub predicate: Predicate,
    pub cause: ObligationCause,
}

#[derive(Clone, Debug)]
pub struct ObligationCause {
    pub span: glyim_span::Span,
    pub code: ObligationCauseCode,
}

#[derive(Clone, Debug)]
pub enum ObligationCauseCode {
    WellFormed,
    TypeConstruction,
    MatchArm,
    IfThenElse,
}

pub struct FulfillmentCtx<'a> {
    pub solver: &'a mut dyn crate::solver::TraitSolver,
    pub ctx: &'a TyCtx,
    obligations: VecDeque<Obligation>,
    processed_count: usize,
    diagnostics: Vec<GlyimDiagnostic>,
}

#[derive(Clone, Debug)]
pub struct OverflowError {
    pub predicate: Predicate,
    pub depth: usize,
}

pub fn can_coerce(ctx: &TyCtx, a: Ty, b: Ty) -> bool {
    if a == b {
        return true;
    }
    match (ctx.ty_kind(a), ctx.ty_kind(b)) {
        (TyKind::Array(elem_a, _), TyKind::Slice(elem_b)) if elem_a == elem_b => true,
        (TyKind::Ref(_, inner_a, mut_a), TyKind::Ref(_, inner_b, mut_b)) => {
            // Allow &mut T -> &T as well
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

impl<'a> FulfillmentCtx<'a> {
    pub fn new(ctx: &'a TyCtx, solver: &'a mut dyn crate::solver::TraitSolver) -> Self {
        Self {
            solver,
            ctx,
            obligations: VecDeque::new(),
            processed_count: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn register_obligation(&mut self, obligation: Obligation) {
        self.obligations.push_back(obligation);
    }

    pub fn process_obligations(&mut self, limit: usize) -> Result<(), OverflowError> {
        while let Some(obligation) = self.obligations.pop_front() {
            self.processed_count += 1;
            if self.processed_count > limit {
                return Err(OverflowError {
                    predicate: obligation.predicate.clone(),
                    depth: self.processed_count,
                });
            }
            match &obligation.predicate {
                Predicate::Trait(trait_pred) => match self.solver.can_prove(self.ctx, trait_pred) {
                    crate::solver::SolverResult::Proven => {}
                    crate::solver::SolverResult::Ambiguous => {
                        self.diagnostics.push(GlyimDiagnostic::type_error(
                            obligation.cause.span,
                            format!("ambiguous trait bound: {:?}", trait_pred),
                        ));
                    }
                    crate::solver::SolverResult::DefiniteNo => {
                        self.diagnostics.push(GlyimDiagnostic::type_error(
                            obligation.cause.span,
                            format!("trait bound not satisfied: {:?}", trait_pred),
                        ));
                    }
                },
                Predicate::WellFormed(_)
                | Predicate::TypeOutlives(_)
                | Predicate::RegionOutlives(_)
                | Predicate::Coerce(_, _) => {}
            }
        }
        Ok(())
    }

    pub fn into_diagnostics(self) -> Vec<GlyimDiagnostic> {
        self.diagnostics
    }
    #[cfg(test)]
    pub(crate) fn pending_count(&self) -> usize {
        self.obligations.len()
    }
    #[cfg(test)]
    pub(crate) fn processed_count(&self) -> usize {
        self.processed_count
    }
}

impl<'a> Extend<Obligation> for FulfillmentCtx<'a> {
    fn extend<T: IntoIterator<Item = Obligation>>(&mut self, iter: T) {
        for ob in iter {
            self.register_obligation(ob);
        }
    }
}
