use glyim_solve::{SolverResult, TraitSolver};
use glyim_type::{Predicate, TraitPredicate, TyCtx};

pub struct MockSolver {
    responses: Vec<(PredicateMatcher, SolverResult)>,
    calls: Vec<TraitPredicate>,
    default: SolverResult,
}

enum PredicateMatcher {
    TraitId(glyim_core::def_id::TraitDefId),
    Any,
}

impl MockSolver {
    pub fn new() -> Self {
        Self {
            responses: Vec::new(),
            calls: Vec::new(),
            default: SolverResult::Ambiguous,
        }
    }
    pub fn default_result(mut self, result: SolverResult) -> Self {
        self.default = result;
        self
    }
    pub fn respond_for_trait(
        mut self,
        id: glyim_core::def_id::TraitDefId,
        result: SolverResult,
    ) -> Self {
        self.responses.push((PredicateMatcher::TraitId(id), result));
        self
    }
    pub fn respond_for_any(mut self, result: SolverResult) -> Self {
        self.responses.push((PredicateMatcher::Any, result));
        self
    }
    pub fn call_count(&self) -> usize {
        self.calls.len()
    }
    pub fn calls(&self) -> &[TraitPredicate] {
        &self.calls
    }

    fn find_response(&self, predicate: &TraitPredicate) -> Option<SolverResult> {
        self.responses.iter().find_map(|(m, r)| match m {
            PredicateMatcher::TraitId(id) if predicate.trait_ref.def_id == *id => Some(r.clone()),
            PredicateMatcher::Any => Some(r.clone()),
            _ => None,
        })
    }
}

impl Default for MockSolver {
    fn default() -> Self {
        Self::new()
    }
}

impl TraitSolver for MockSolver {
    fn can_prove(&mut self, _ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult {
        self.calls.push(predicate.clone());
        self.find_response(predicate)
            .unwrap_or_else(|| self.default.clone())
    }
    fn evaluate_predicate(&mut self, ctx: &TyCtx, predicate: &Predicate) -> SolverResult {
        match predicate {
            Predicate::Trait(tp) => self.can_prove(ctx, tp),
            _ => self.default.clone(),
        }
    }
}
