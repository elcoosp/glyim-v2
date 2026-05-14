//! Type checker: HIR → THIR with full inference and trait solving.
//!
//! `TypeckCtx` does NOT hold a solver reference. Obligations are
//! collected into a `Vec<Obligation>` during type-checking, then
//! processed by `FulfillmentCtx` after each body is checked.

mod check_body;
pub mod thir;

use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::primitives::Mutability;
use glyim_diag::GlyimDiagnostic;
use glyim_solve::{FulfillmentCtx, InferenceTable, Obligation, ObligationCause};
use glyim_span::Span;
use glyim_type::*;

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
            Err(diags) => {
                self.diagnostics.extend(diags);
                false
            }
        }
    }

    pub fn require_trait_bound(&mut self, ty: Ty, trait_pred: TraitPredicate, span: Span) {
        let _ = ty;
        self.pending_obligations.push(Obligation {
            predicate: Predicate::Trait(trait_pred),
            cause: ObligationCause {
                span,
                code: glyim_solve::ObligationCauseCode::TypeConstruction,
            },
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

    let expr_types: IndexVec<glyim_hir::ExprId, Option<Ty>> = IndexVec::new();
    let pat_types: IndexVec<glyim_hir::PatId, Option<Ty>> = IndexVec::new();
    let adjustments: IndexVec<glyim_hir::ExprId, Option<Vec<Adjustment>>> = IndexVec::new();
    let mut thir_bodies: Vec<(LocalDefId, thir::Body)> = Vec::new();

    for (_item_id, item) in hir.items.iter_enumerated() {
        let body_info: Option<(glyim_hir::BodyId, u32, Vec<glyim_hir::Param>, Ty)> =
            match &item.kind {
                glyim_hir::ItemKind::Fn(f) => {
                    let ret_ty = match &f.return_ty {
                        Some(_) => {
                            let var = infer.new_ty_var(&mut ctx);
                            ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
                        }
                        None => Ty::UNIT,
                    };
                    f.body
                        .map(|b| (b, item.id.to_raw(), f.params.clone(), ret_ty))
                }
                _ => None,
            };

        if let Some((body_id, local_def_raw, params, return_ty)) = body_info {
            let local_def_id = LocalDefId::from_raw(local_def_raw);
            let mut pending = Vec::new();
            let thir_body = check_body::check_function_body(
                &mut ctx,
                &mut infer,
                &mut diagnostics,
                &mut pending,
                body_id,
                hir,
                local_def_id,
                return_ty,
                &params,
            );
            all_obligations.extend(pending);
            thir_bodies.push((local_def_id, thir_body));
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
        expr_types,
        pat_types,
        adjustments,
        thir_bodies,
        diagnostics,
    };

    (frozen_ctx, result)
}

#[cfg(test)]
mod tests;
