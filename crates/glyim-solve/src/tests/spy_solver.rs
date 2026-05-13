use glyim_solve::{SolverResult, TraitSolver};
use glyim_type::*;

/// Spy solver that records all calls to `can_prove` and returns predetermined responses.
pub(crate) struct SpySolver {
    pub calls: Vec<TraitPredicate>,
    responses: Vec<SolverResult>,
    default_response: SolverResult,
}

impl SpySolver {
    pub fn new(default: SolverResult) -> Self {
        SpySolver {
            calls: Vec::new(),
            responses: Vec::new(),
            default_response: default,
        }
    }

    /// Push a response that will be returned by the next call (LIFO order).
    pub fn respond_with(&mut self, responses: Vec<SolverResult>) {
        self.responses.extend(responses.into_iter().rev());
    }
}

impl TraitSolver for SpySolver {
    fn can_prove(&mut self, _ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult {
        self.calls.push(predicate.clone());
        self.responses.pop().unwrap_or_else(|| self.default_response.clone())
    }

    fn evaluate_predicate(&mut self, _ctx: &TyCtx, _predicate: &Predicate) -> SolverResult {
        unimplemented!("SpySolver::evaluate_predicate not needed for these tests")
    }
}
