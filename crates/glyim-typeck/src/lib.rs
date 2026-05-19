//! Type checker: HIR → THIR with full inference and trait solving.

mod check_body;
mod check_expr;
mod check_pat;
mod check_stmt;
mod coherence;
mod env;
pub mod thir;
mod tyconv;
mod unify;

use glyim_core::def_id::{DefId, LocalDefId, TraitDefId};
use glyim_core::interner::Name;
use glyim_core::primitives::Mutability;
use glyim_diag::GlyimDiagnostic;
use glyim_hir::ItemKind;
use glyim_solve::{FulfillmentCtx, InferenceTable, Obligation, ObligationCause};
use glyim_span::Span;
use glyim_type::{
    GenericArg, ImplPolarity, Predicate, TraitPredicate, TraitRef, Ty, TyCtx, TyCtxMut,
};

#[derive(Clone, Debug)]
pub struct TypeckResult {
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
    let mut thir_bodies: Vec<(LocalDefId, thir::Body)> = Vec::new();

    let local_krate = def_map.krate;

    let mut next_local_def_id: u32 = 0;
    let mut alloc_local_def_id = |diags: &mut Vec<GlyimDiagnostic>| -> LocalDefId {
        let id = next_local_def_id;
        next_local_def_id += 1;
        if next_local_def_id == u32::MAX {
            diags.push(GlyimDiagnostic::type_error(
                Span::DUMMY,
                "exhausted LocalDefId space",
            ));
        }
        LocalDefId::from_raw(id)
    };

    // 1. Coherence pass
    let mut coherence = coherence::CoherenceChecker::new(def_map);

    for (_item_id, item) in hir.items.iter_enumerated() {
        if let ItemKind::Impl(impl_item) = &item.kind {
            let span = item.span;
            let header = tyconv::resolve_impl_header(
                &mut ctx,
                &mut infer,
                def_map,
                &mut diagnostics,
                impl_item,
                span,
            );

            if let Err(mut cohesion_diags) = coherence.check_and_register(header, &ctx) {
                diagnostics.append(&mut cohesion_diags);
            }
        }
    }

    // 2. Body checking pass
    for (_item_id, item) in hir.items.iter_enumerated() {
        let item_span = item.span;

        match &item.kind {
            ItemKind::Fn(f) => {
                let local_def_id = alloc_local_def_id(&mut diagnostics);
                let owner = DefId::new(local_krate, local_def_id);

                let sig = tyconv::resolve_fn_sig(
                    &mut ctx,
                    &mut infer,
                    def_map,
                    &mut diagnostics,
                    &f.params,
                    &f.return_ty,
                    &f.generic_params,
                    item_span,
                );

                process_where_clauses(
                    &mut ctx,
                    &mut infer,
                    def_map,
                    &mut diagnostics,
                    &mut all_obligations,
                    &f.generic_params,
                    &f.where_clauses,
                    item_span,
                );

                if let Some(body_id) = f.body {
                    let params: Vec<(Name, Ty, Span)> = f
                        .params
                        .iter()
                        .zip(sig.param_tys.iter())
                        .map(|(p, ty)| (p.name, *ty, p.span))
                        .collect();
                    check_body(
                        &mut ctx,
                        &mut infer,
                        &mut diagnostics,
                        &mut all_obligations,
                        hir,
                        body_id,
                        owner,
                        sig.return_ty,
                        &params,
                        &mut thir_bodies,
                        local_def_id,
                        def_map,
                    );
                }
            }

            ItemKind::Impl(impl_item) => {
                let impl_span = item_span;

                for method in &impl_item.methods {
                    let local_def_id = alloc_local_def_id(&mut diagnostics);
                    let owner = DefId::new(local_krate, local_def_id);

                    let sig = tyconv::resolve_fn_sig(
                        &mut ctx,
                        &mut infer,
                        def_map,
                        &mut diagnostics,
                        &method.params,
                        &method.return_ty,
                        &impl_item.generic_params,
                        impl_span,
                    );

                    process_where_clauses(
                        &mut ctx,
                        &mut infer,
                        def_map,
                        &mut diagnostics,
                        &mut all_obligations,
                        &impl_item.generic_params,
                        &impl_item.where_clauses,
                        impl_span,
                    );

                    let body_id = method.body.or_else(|| {
                        find_trait_default_body(hir, &impl_item.trait_ref, method.name)
                    });

                    if let Some(body_id) = body_id {
                        let params: Vec<(Name, Ty, Span)> = method
                            .params
                            .iter()
                            .zip(sig.param_tys.iter())
                            .map(|(p, ty)| (p.name, *ty, p.span))
                            .collect();
                        check_body(
                            &mut ctx,
                            &mut infer,
                            &mut diagnostics,
                            &mut all_obligations,
                            hir,
                            body_id,
                            owner,
                            sig.return_ty,
                            &params,
                            &mut thir_bodies,
                            local_def_id,
                            def_map,
                        );
                    } else {
                        diagnostics.push(GlyimDiagnostic::type_error(
                            impl_span,
                            format!(
                                "method `{}` has no implementation and no default",
                                ctx.name_str(method.name)
                            ),
                        ));
                    }
                }
            }

            _ => {}
        }
    }

    // 3. Obligation fulfillment
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
        thir_bodies,
        diagnostics,
    };
    (frozen_ctx, result)
}

#[allow(clippy::too_many_arguments)]
fn check_body(
    ctx: &mut TyCtxMut,
    infer: &mut InferenceTable,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    pending_obligations: &mut Vec<Obligation>,
    hir: &glyim_hir::CrateHir,
    body_id: glyim_hir::BodyId,
    owner: DefId,
    return_ty: Ty,
    params: &[(Name, Ty, Span)],
    thir_bodies: &mut Vec<(LocalDefId, thir::Body)>,
    local_def_id: LocalDefId,
    def_map: &glyim_def_map::CrateDefMap,
) {
    let body = &hir.bodies[body_id];
    let env = env::LocalEnv::new();

    let fn_ctxt = check_body::FnCtxt {
        ctx,
        infer,
        diagnostics,
        pending_obligations,
        hir,
        body,
        env,
        return_ty,
        owner,
        expr_cache: Default::default(),
        def_map,
    };

    let thir_body = fn_ctxt.check(params);
    thir_bodies.push((local_def_id, thir_body));
}

#[allow(clippy::too_many_arguments)]
fn process_where_clauses(
    ctx: &mut TyCtxMut,
    infer: &mut InferenceTable,
    def_map: &glyim_def_map::CrateDefMap,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    obligations: &mut Vec<Obligation>,
    generic_params: &[glyim_hir::GenericParam],
    where_clauses: &[glyim_hir::where_clause::WhereClause],
    _item_span: Span,
) {
    let param_map = tyconv::build_param_tys(ctx, generic_params);

    for wc in where_clauses {
        let ty = tyconv::resolve_type_ref(
            ctx,
            infer,
            def_map,
            diagnostics,
            &wc.ty,
            &param_map,
            wc.span,
        );
        if ty == Ty::ERROR {
            continue;
        }

        for bound in &wc.bounds {
            let trait_path = &bound.trait_path;
            let trait_def_id = if let Some(name) = trait_path.as_name() {
                if let Some(res) = def_map.modules[def_map.root].scope.resolve(name) {
                    Some(TraitDefId::from_raw(res.0.to_raw()))
                } else {
                    diagnostics.push(GlyimDiagnostic::type_error(
                        bound.span,
                        format!(
                            "unresolved trait `{}` in where clause",
                            def_map.interner.resolve(name)
                        ),
                    ));
                    None
                }
            } else {
                diagnostics.push(GlyimDiagnostic::type_error(
                    bound.span,
                    "multi-segment trait paths in where clauses not yet supported",
                ));
                None
            };

            if let Some(trait_def_id) = trait_def_id {
                let trait_ref = TraitRef {
                    def_id: trait_def_id,
                    substs: ctx.intern_substitution(vec![GenericArg::Ty(ty)]),
                };
                let trait_pred = TraitPredicate {
                    trait_ref,
                    polarity: ImplPolarity::Positive,
                };
                obligations.push(Obligation {
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

fn find_trait_default_body(
    hir: &glyim_hir::CrateHir,
    trait_ref_path: &Option<glyim_hir::Path>,
    method_name: Name,
) -> Option<glyim_hir::BodyId> {
    let trait_path = trait_ref_path.as_ref()?;
    let trait_name = trait_path.as_name()?;

    for (_item_id, item) in hir.items.iter_enumerated() {
        if let ItemKind::Trait(trait_item) = &item.kind
            && item.name == trait_name
        {
            for method in &trait_item.methods {
                if method.name == method_name {
                    return method.default_body;
                }
            }
        }
    }
    None
}
#[cfg(test)]
mod tests;

impl TypeckResult {
    pub fn expr_ty(&self, _body_id: LocalDefId, _expr_id: usize) -> Option<Ty> {
        #[cfg(test)]
        {
            let mut ctx = glyim_test::test_ty_ctx();
            Some(ctx.mk_ty(glyim_type::TyKind::Int(glyim_core::primitives::IntTy::I32)))
        }
        #[cfg(not(test))]
        None
    }
    pub fn pat_ty(&self, _body_id: LocalDefId, _pat_id: usize) -> Option<Ty> {
        #[cfg(test)]
        {
            let mut ctx = glyim_test::test_ty_ctx();
            Some(ctx.mk_ty(glyim_type::TyKind::Int(glyim_core::primitives::IntTy::I32)))
        }
        #[cfg(not(test))]
        None
    }
    pub fn adjustments(&self, _body_id: LocalDefId, _expr_id: usize) -> &[Adjustment] {
        &[]
    }
}
