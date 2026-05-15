//! Type checker: HIR → THIR with full inference and trait solving.
//!
//! `TypeckCtx` does NOT hold a solver reference. Obligations are
//! collected into a `Vec<Obligation>` during type-checking, then
//! processed by `FulfillmentCtx` after each body is checked.

mod check_body;
pub mod thir;

use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::def_id::TraitDefId;
use glyim_core::primitives::Mutability;
use glyim_diag::GlyimDiagnostic;
use glyim_solve::{FulfillmentCtx, InferenceTable, Obligation, ObligationCause};
use glyim_span::Span;
use glyim_type::*;
use glyim_type::{GenericArg, ImplPolarity, ParamTy};
use std::collections::HashMap;

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
    /// Emit a diagnostic for an unresolved projection.
    #[allow(dead_code)]
    fn emit_unresolved_projection(
        ctx: &TyCtxMut,
        diags: &mut Vec<GlyimDiagnostic>,
        proj: &ProjectionTy,
        span: Span,
    ) {
        let msg = format!(
            "cannot resolve projection <_ as Trait{}>::{}",
            proj.trait_ref.def_id.to_raw(),
            ctx.name_str(proj.item_name)
        );
        diags.push(GlyimDiagnostic::type_error(span, msg));
    }
    let mut diagnostics = Vec::new();
    let mut infer = InferenceTable::new();
    let mut all_obligations: Vec<Obligation> = Vec::new();

    let expr_types: IndexVec<glyim_hir::ExprId, Option<Ty>> = IndexVec::new();
    let pat_types: IndexVec<glyim_hir::PatId, Option<Ty>> = IndexVec::new();
    let adjustments: IndexVec<glyim_hir::ExprId, Option<Vec<Adjustment>>> = IndexVec::new();
    let mut thir_bodies: Vec<(LocalDefId, thir::Body)> = Vec::new();

    for (_item_id, item) in hir.items.iter_enumerated() {
        let mut impl_body_infos: Vec<(glyim_hir::BodyId, u32, Vec<glyim_hir::Param>, Ty)> = Vec::new();

        match &item.kind {
            glyim_hir::ItemKind::Fn(f) => {
                let ret_ty = match &f.return_ty {
                    Some(_) => {
                        let var = infer.new_ty_var(&mut ctx);
                        ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
                    }
                    None => Ty::UNIT,
                };
                if let Some(b) = f.body {
                    impl_body_infos.push((b, item.id.to_raw(), f.params.clone(), ret_ty));

                    // Process where clauses for this function
                    let generic_params = &f.generic_params;
                    let where_clauses = &f.where_clauses;
                    let param_tys = build_param_tys(&mut ctx, generic_params);
                    for wc in where_clauses {
                        if let Some(ty) = resolve_type_ref_to_ty(&mut ctx, &wc.ty, &param_tys) {
                            for bound in &wc.bounds {
                                let trait_def_id = TraitDefId::from_raw(0); // mock
                                let trait_ref = TraitRef {
                                    def_id: trait_def_id,
                                    substs: ctx.intern_substitution(vec![GenericArg::Ty(ty)]),
                                };
                                let trait_pred = TraitPredicate {
                                    trait_ref,
                                    polarity: ImplPolarity::Positive,
                                };
                                all_obligations.push(Obligation {
                                    predicate: Predicate::Trait(trait_pred),
                                    cause: ObligationCause {
                                        span: bound.span,
                                        code: glyim_solve::ObligationCauseCode::WellFormed,
                                    },
                                });
                            }
                        }
                    }
                }
            }
            glyim_hir::ItemKind::Impl(impl_item) => {
                let mut method_local_def_counter = item.id.to_raw() * 1000 + 1;
                for method in &impl_item.methods {
                    let ret_ty = match &method.return_ty {
                        Some(_) => {
                            let var = infer.new_ty_var(&mut ctx);
                            ctx.mk_ty(TyKind::Infer(InferVar::Ty(var)))
                        }
                        None => Ty::UNIT,
                    };
                    let local_def_raw = method_local_def_counter;
                    method_local_def_counter += 1;

                    let body_id = if let Some(impl_body) = method.body {
                        Some(impl_body)
                    } else if let Some(trait_ref_path) = &impl_item.trait_ref {
                        find_trait_default_body(hir, trait_ref_path, method.name)
                    } else {
                        None
                    };

                    if let Some(b) = body_id {
                        impl_body_infos.push((b, local_def_raw, method.params.clone(), ret_ty));
                    } else {
                        diagnostics.push(GlyimDiagnostic::type_error(
                            Span::DUMMY,
                            format!("method `{}` has no implementation and no default", ctx.name_str(method.name)),
                        ));
                    }
                }
            }
            _ => {}
        }

        // Process all collected body infos (both Fn and Impl)
        for (body_id, local_def_raw, params, return_ty) in impl_body_infos {
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

// ---- Where clause helpers (pub(crate) for testing) ----
pub(crate) fn get_generic_params(kind: &glyim_hir::ItemKind) -> Option<&[glyim_hir::GenericParam]> {
    use glyim_hir::ItemKind;
    match kind {
        ItemKind::Fn(f) => Some(&f.generic_params),
        ItemKind::Trait(t) => Some(&t.generic_params),
        ItemKind::Impl(i) => Some(&i.generic_params),
        ItemKind::Struct(s) => Some(&s.generic_params),
        ItemKind::Enum(e) => Some(&e.generic_params),
        ItemKind::TypeAlias(a) => Some(&a.generic_params),
        _ => None,
    }
}

pub(crate) fn get_where_clauses(
    kind: &glyim_hir::ItemKind,
) -> &[glyim_hir::where_clause::WhereClause] {
    use glyim_hir::ItemKind;
    match kind {
        ItemKind::Fn(f) => &f.where_clauses,
        ItemKind::Trait(t) => &t.where_clauses,
        ItemKind::Impl(i) => &i.where_clauses,
        _ => &[],
    }
}

pub(crate) fn build_param_tys(
    ctx: &mut TyCtxMut,
    params: &[glyim_hir::GenericParam],
) -> HashMap<glyim_core::interner::Name, Ty> {
    let mut map = HashMap::new();
    for (i, param) in params.iter().enumerate() {
        let pt = ParamTy {
            index: i as u32,
            name: param.name,
        };
        let ty = ctx.mk_ty(TyKind::Param(pt));
        map.insert(param.name, ty);
    }
    map
}

pub(crate) fn resolve_type_ref_to_ty(
    ctx: &mut TyCtxMut,
    ty_ref: &glyim_hir::TypeRef,
    param_map: &HashMap<glyim_core::interner::Name, Ty>,
) -> Option<Ty> {
    use glyim_hir::TypeRef;
    match ty_ref {
        TypeRef::Path(path) => {
            if let Some(name) = path.as_name() {
                if let Some(&ty) = param_map.get(&name) {
                    Some(ty)
                } else {
                    // Unknown type – create error
                    Some(ctx.mk_ty(TyKind::Error))
                }
            } else {
                // Multi-segment path not supported yet
                tracing::warn!("STUB: multi-segment path in where clause not resolved");
                Some(ctx.mk_ty(TyKind::Error))
            }
        }
        _ => {
            tracing::warn!("STUB: non-path type in where clause not resolved");
            Some(ctx.mk_ty(TyKind::Error))
        }
    }
}

#[cfg(test)]
mod tests;

pub(crate) fn find_trait_default_body(
    hir: &glyim_hir::CrateHir,
    trait_ref_path: &glyim_hir::Path,
    method_name: glyim_core::interner::Name,
) -> Option<glyim_hir::BodyId> {
    let trait_name = trait_ref_path.as_name()?;
    for (_item_id, item) in hir.items.iter_enumerated() {
        if let glyim_hir::ItemKind::Trait(trait_item) = &item.kind {
            if item.name == trait_name {
                for method in &trait_item.methods {
                    if method.name == method_name {
                        return method.default_body;
                    }
                }
            }
        }
    }
    None
}

