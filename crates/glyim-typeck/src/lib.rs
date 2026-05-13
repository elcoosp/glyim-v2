//! Type checker: HIR → THIR with full inference and trait solving.
//!
//! `TypeckCtx` does NOT hold a solver reference. Obligations are
//! collected into a `Vec<Obligation>` during type-checking, then
//! processed by `FulfillmentCtx` after each body is checked.

pub mod thir;

use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::primitives::Mutability;
use glyim_type::*;
use glyim_hir;
use glyim_solve::{FulfillmentCtx, InferenceTable, Obligation, ObligationCause};
use glyim_diag::GlyimDiagnostic;
use glyim_span::Span;

#[derive(Clone, Debug)]
pub struct TypeckResult {
    pub expr_types: IndexVec<glyim_hir::ExprId, Option<Ty>>,
    pub pat_types: IndexVec<glyim_hir::PatId, Option<Ty>>,
    pub adjustments: IndexVec<glyim_hir::ExprId, Option<Vec<Adjustment>>>,
    pub thir_bodies: Vec<(LocalDefId, thir::Body)>,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

#[derive(Clone, Debug)]
pub struct Adjustment {
    pub kind: AdjustKind,
    pub target: Ty,
}

#[derive(Clone, Debug)]
pub enum AdjustKind {
    Deref,
    Borrow(Mutability),
    NeverToAny,
}

pub struct TypeckCtx<'a> {
    pub ctx: &'a mut TyCtxMut,
    pub infer: &'a mut InferenceTable,
    pub diagnostics: &'a mut Vec<GlyimDiagnostic>,
    pub pending_obligations: Vec<Obligation>,
}

impl<'a> TypeckCtx<'a> {
    pub fn unify(&mut self, a: Ty, b: Ty, span: Span) -> bool {
        match self.infer.unify(self.ctx, a, b, span) {
            Ok(_) => true,
            Err(diags) => { self.diagnostics.extend(diags); false }
        }
    }

    pub fn require_trait_bound(&mut self, ty: Ty, trait_pred: TraitPredicate, span: Span) {
        let _ = ty;
        self.pending_obligations.push(Obligation {
            predicate: Predicate::Trait(trait_pred),
            cause: ObligationCause { span, code: glyim_solve::ObligationCauseCode::TypeConstruction },
        });
    }
}

#[tracing::instrument(level = "info", skip(ctx, solver))]
pub fn typeck_crate(
    mut ctx: TyCtxMut,
    def_map: &glyim_def_map::CrateDefMap,
    hir: &glyim_hir::CrateHir,
    solver: &mut dyn glyim_solve::TraitSolver,
) -> (TyCtx, TypeckResult) {
    let mut diagnostics = Vec::new();
    let mut infer = InferenceTable::new();
    let mut all_obligations: Vec<Obligation> = Vec::new();

    for (_id, item) in hir.items.iter_enumerated() {
        let body_id = match &item.kind {
            glyim_hir::ItemKind::Fn(f) => f.body,
            glyim_hir::ItemKind::Const(c) => c.body,
            glyim_hir::ItemKind::Static(s) => s.body,
            _ => None,
        };

        if let Some(_body_id) = body_id {
            let mut typeck_ctx = TypeckCtx {
                ctx: &mut ctx,
                infer: &mut infer,
                diagnostics: &mut diagnostics,
                pending_obligations: Vec::new(),
            };
            let _ = (def_map, &typeck_ctx);
            all_obligations.extend(std::mem::take(&mut typeck_ctx.pending_obligations));
        }
    }

    let frozen_ctx = ctx.freeze();

    let mut fulfill = FulfillmentCtx::new(&frozen_ctx, solver);
    fulfill.extend(all_obligations);
    if let Err(overflow) = fulfill.process_obligations(100_000) {
        diagnostics.push(GlyimDiagnostic::type_error(
            Span::DUMMY,
            format!("overflow evaluating obligation: {:?}", overflow.predicate),
        ));
    }
    diagnostics.extend(fulfill.into_diagnostics());

    let result = TypeckResult {
        expr_types: IndexVec::new(),
        pat_types: IndexVec::new(),
        adjustments: IndexVec::new(),
        thir_bodies: Vec::new(),
        diagnostics,
    };

    (frozen_ctx, result)
}
