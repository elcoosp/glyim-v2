//! Type checker: HIR → THIR with full inference and trait solving.
#![allow(missing_docs)]
// Stylistic clippy lints suppressed crate-wide (test-noise lints).
#![allow(
    clippy::cloned_ref_to_slice_refs,
    clippy::vec_init_then_push,
    clippy::assertions_on_constants,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::manual_c_str_literals,
    clippy::doc_lazy_continuation,
    clippy::empty_line_after_doc_comments,
    clippy::manual_strip,
    clippy::needless_range_loop,
    clippy::unnecessary_cast,
    clippy::clone_on_copy,
    clippy::mutable_key_type,
    clippy::only_used_in_recursion,
    clippy::let_unit_value,
    clippy::unnecessary_literal_unwrap,
    clippy::format_in_format_args,
    clippy::permissions_set_readonly_false,
    clippy::needless_lifetimes,
    clippy::collapsible_if
)]

mod check_body;
mod check_expr;
mod check_pat;
mod check_stmt;
mod coherence;
mod env;
pub mod thir;
mod tyconv;
mod unify;

use std::collections::HashMap;

use glyim_core::arena::IndexVec;
use glyim_core::def_id::{AdtId, ConstDefId, CrateId, DefId, FnDefId, LocalDefId};
use glyim_core::interner::Name;
use glyim_core::primitives::{Abi, Mutability, Safety};
use glyim_diag::GlyimDiagnostic;
use glyim_hir::{ItemId, ItemKind, ExprId};
use glyim_def_map::{CrateDefMap, ModuleId};
use glyim_solve::{FulfillmentCtx, InferenceTable, Obligation, ObligationCause, TraitContext};
use glyim_span::Span;
use glyim_type::{
    AdtDef, AdtKind, FieldDef, FieldIdx, GenericArg, ImplPolarity, Predicate, TraitPredicate,
    TraitRef, Ty, TyCtx, TyCtxMut, VariantDef, FnSig,
};

#[derive(Clone, Debug)]
pub struct TypeckResult {
    pub thir_bodies: Vec<(LocalDefId, thir::Body)>,
    pub diagnostics: Vec<GlyimDiagnostic>,
    /// Evaluated values of constant definitions (Part C: const value
    /// materialization). Populated during `typeck_crate` by const-evaluating
    /// each `const` initializer; consumed by MIR lowering (via `LowerCtx::
    /// const_value`) to fold `ConstRef` into a concrete `MirConstKind`.
    pub const_values: HashMap<ConstDefId, glyim_const_eval::ConstValue>,
    /// Resolved type of each HIR expression, keyed by the owning function's
    /// `LocalDefId` and the expression's `ExprId`. Populated during
    /// `typeck_crate` from each `FnCtxt::expr_cache` (which records the type
    /// of every checked expression) after inference variables are fully
    /// resolved. Drives downstream consumers such as LSP completion
    /// filtering (Tier 6.4).
    pub expr_types: HashMap<LocalDefId, HashMap<ExprId, Ty>>,
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
    let trait_ctx = TraitContext::new();
    let mut thir_bodies: Vec<(LocalDefId, thir::Body)> = Vec::new();
    let mut all_expr_types: HashMap<LocalDefId, HashMap<ExprId, Ty>> = HashMap::new();
    let mut all_const_values: HashMap<ConstDefId, glyim_const_eval::ConstValue> = HashMap::new();

    let local_krate = def_map.krate;

    let mut next_local_def_id: u32 = 0;
    let mut alloc_local_def_id = |counter: &mut u32, diags: &mut Vec<GlyimDiagnostic>| -> LocalDefId {
        let id = *counter;
        *counter += 1;
        if *counter == u32::MAX {
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

            if let Err(mut cohesion_diags) =
                coherence.check_and_register(header, &mut ctx, &mut infer)
            {
                diagnostics.append(&mut cohesion_diags);
            }
        }
    }

    // 2. Body checking pass
    // Gather ItemIds that are children of a ModItem so the flat pass skips
    // them — they are handled by the recursive fn walker below, which tracks
    // the def-map module context for correct LocalDefId alignment.
    let mut child_set: std::collections::HashSet<ItemId> =
        std::collections::HashSet::new();
    for (_id, item) in hir.items.iter_enumerated() {
        if let ItemKind::Mod(m) = &item.kind {
            for c in &m.children {
                child_set.insert(*c);
            }
        }
    }

    for (item_id, item) in hir.items.iter_enumerated() {
        if child_set.contains(&item_id) {
            continue;
        }
        let item_span = item.span;

        match &item.kind {
            ItemKind::Fn(_) => {}

            ItemKind::Impl(impl_item) => {
                let impl_span = item_span;

                for method in &impl_item.methods {
                    let local_def_id = alloc_local_def_id(&mut next_local_def_id, &mut diagnostics);
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

                    // Register the resolved signature for the LLVM codegen pass
                    // (see the Fn arm for rationale).
                    let inputs = ctx.intern_substitution(
                        sig.param_tys.iter().map(|t| GenericArg::Ty(*t)).collect(),
                    );
                    ctx.register_fn_sig(
                        FnDefId::from_raw(local_def_id.to_raw()),
                        FnSig {
                            inputs,
                            output: sig.return_ty,
                            c_variadic: false,
                            unsafety: Safety::Safe,
                            abi: Abi::Glyim,
                        },
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
                            &trait_ctx,
                            &mut all_expr_types,
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

    // Function definitions (top-level and nested in `mod`) are registered
    // under the def-map's LocalDefId so value-namespace path resolution lines
    // up with the id `check_path` derives from the def map.
    let top_level_ids: Vec<ItemId> = hir
        .items
        .iter_enumerated()
        .filter(|(id, _)| !child_set.contains(id))
        .map(|(id, _)| id)
        .collect();
    check_fn_items_in_module(
        &mut ctx,
        &mut infer,
        def_map,
        &mut diagnostics,
        &mut all_obligations,
        hir,
        local_krate,
        &trait_ctx,
        &mut all_expr_types,
        &mut thir_bodies,
        &mut all_const_values,
        &top_level_ids,
        def_map.root,
        &mut next_local_def_id,
    );

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

    // Resolve inference variables in the collected per-expression types so the
    // public `expr_ty` query returns concrete types, not `TyKind::Infer(..)`.
    let mut expr_types: HashMap<LocalDefId, HashMap<ExprId, Ty>> = HashMap::new();
    for (body_id, raw) in &all_expr_types {
        let mut resolved: HashMap<ExprId, Ty> = HashMap::new();
        for (eid, ty) in raw {
            let ty = infer
                .fully_resolve(&frozen_ctx, *ty)
                .unwrap_or(*ty);
            resolved.insert(*eid, ty);
        }
        expr_types.insert(*body_id, resolved);
    }

    let result = TypeckResult {
        thir_bodies,
        diagnostics,
        const_values: all_const_values.clone(),
        expr_types,
    };
    (frozen_ctx, result)
}

/// Recursively type-check function definitions, walking the HIR `ModItem`
/// tree in lockstep with the def-map module tree.
///
/// Each function is registered under the def-map's `LocalDefId` (looked up by
/// name within its enclosing module), so the `FnDefId` used here matches the
/// one `check_path` derives from the def map when resolving a value-namespace
/// path (`foo`, `mod::foo`). The flat body-check loop in `typeck_crate` skips
/// `ModItem` children and delegates them here.
#[allow(clippy::too_many_arguments)]
fn check_fn_items_in_module(
    ctx: &mut TyCtxMut,
    infer: &mut InferenceTable,
    def_map: &CrateDefMap,
    diagnostics: &mut Vec<GlyimDiagnostic>,
    all_obligations: &mut Vec<Obligation>,
    hir: &glyim_hir::CrateHir,
    local_krate: CrateId,
    trait_ctx: &TraitContext,
    all_expr_types: &mut HashMap<LocalDefId, HashMap<ExprId, Ty>>,
    thir_bodies: &mut Vec<(LocalDefId, thir::Body)>,
    all_const_values: &mut HashMap<ConstDefId, glyim_const_eval::ConstValue>,
    item_ids: &[ItemId],
    module_id: ModuleId,
    next_local_def_id: &mut u32,
) {
    for item_id in item_ids {
        let item = match hir.items.get(*item_id) {
            Some(i) => i,
            None => continue,
        };
        let item_span = item.span;
        match &item.kind {
            ItemKind::Fn(f) => {
                // The def map is the source of truth for a function's
                // LocalDefId; resolve it by name within the current module.
                // Functions not present in the def-map scope (e.g. `main`,
                // which the def-map treats specially) fall back to the legacy
                // per-item counter so they still get checked.
                let local_def_id = match def_map.modules[module_id]
                    .scope
                    .values
                    .get(&item.name)
                {
                    Some((id, _, _)) => *id,
                    // Fall back to the HIR `item.id` when the def-map does not
                    // track this item (e.g. `main`). This must match the id
                    // `discover_mono_roots` assigns to entry points so the
                    // MIR body is found during monomorphization.
                    None => LocalDefId::from_raw(item.id.to_raw()),
                };
                let owner = DefId::new(local_krate, local_def_id);

                let sig = tyconv::resolve_fn_sig(
                    ctx,
                    infer,
                    def_map,
                    diagnostics,
                    &f.params,
                    &f.return_ty,
                    &f.generic_params,
                    item_span,
                );

                let inputs = ctx.intern_substitution(
                    sig.param_tys.iter().map(|t| GenericArg::Ty(*t)).collect(),
                );
                ctx.register_fn_sig(
                    FnDefId::from_raw(local_def_id.to_raw()),
                    FnSig {
                        inputs,
                        output: sig.return_ty,
                        c_variadic: false,
                        unsafety: Safety::Safe,
                        abi: Abi::Glyim,
                    },
                );

                process_where_clauses(
                    ctx,
                    infer,
                    def_map,
                    diagnostics,
                    all_obligations,
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
                        ctx,
                        infer,
                        diagnostics,
                        all_obligations,
                        hir,
                        body_id,
                        owner,
                        sig.return_ty,
                        &params,
                        thir_bodies,
                        local_def_id,
                        def_map,
                        trait_ctx,
                        all_expr_types,
                    );
                }
            }
            ItemKind::Enum(enum_item) => {
                // Register the enum as an ADT so variant field types are
                // available for data-carrying variant constructors and pattern
                // matching. Resolve the enum's `LocalDefId` from the current
                // module's type namespace (works for both top-level and
                // module-nested enums).
                let enum_local = def_map
                    .modules
                    .get(module_id)
                    .and_then(|m| m.scope.types.get(&item.name))
                    .map(|(id, _, _)| *id);
                if let Some(enum_local) = enum_local {
                    let adt_id = AdtId::from_raw(enum_local.to_raw());
                    let mut variants = Vec::new();
                    for variant in &enum_item.variants {
                        let mut fields = IndexVec::new();
                        for field in &variant.fields {
                            let field_ty = tyconv::resolve_type_ref(
                                ctx,
                                infer,
                                def_map,
                                diagnostics,
                                &field.ty,
                                &HashMap::new(),
                                field.span,
                            );
                            fields.push(FieldDef {
                                name: field.name,
                                ty: field_ty,
                            });
                        }
                        variants.push(VariantDef {
                            name: variant.name,
                            fields,
                        });
                    }
                    ctx.register_adt(
                        adt_id,
                        AdtDef {
                            kind: AdtKind::Enum,
                            fields: IndexVec::new(),
                            variants,
                        },
                    );
                }
            }
            ItemKind::Struct(struct_item) => {
                // Register the struct as an ADT so its field types are
                // available for struct-literal typing, field access, and
                // pattern destructuring. Resolve the struct's `LocalDefId`
                // from the current module's type namespace.
                let struct_local = def_map
                    .modules
                    .get(module_id)
                    .and_then(|m| m.scope.types.get(&item.name))
                    .map(|(id, _, _)| *id);
                if let Some(struct_local) = struct_local {
                    let adt_id = AdtId::from_raw(struct_local.to_raw());
                    let mut fields = IndexVec::new();
                    for field in &struct_item.fields {
                        let field_ty = tyconv::resolve_type_ref(
                            ctx,
                            infer,
                            def_map,
                            diagnostics,
                            &field.ty,
                            &HashMap::new(),
                            field.span,
                        );
                        fields.push(FieldDef {
                            name: field.name,
                            ty: field_ty,
                        });
                    }
                    ctx.register_adt(
                        adt_id,
                        AdtDef {
                            kind: AdtKind::Struct,
                            fields,
                            variants: Vec::new(),
                        },
                    );
                }
            }
            ItemKind::Mod(m) => {
                let child_mod = def_map.modules[module_id]
                    .children
                    .iter()
                    .find(|(n, _)| *n == item.name)
                    .map(|(_, id)| *id);
                if let Some(child_mod) = child_mod {
                    check_fn_items_in_module(
                        ctx,
                        infer,
                        def_map,
                        diagnostics,
                        all_obligations,
                        hir,
                        local_krate,
                        trait_ctx,
                        all_expr_types,
                        thir_bodies,
                        all_const_values,
                        &m.children,
                        child_mod,
                        next_local_def_id,
                    );
                }
            }
            ItemKind::Const(c) => {
                // Resolve the constant's value type and register it so
                // `check_path` can produce a `ConstRef` with the right type.
                // The constant's body is not yet evaluated/threaded to codegen
                // (const value materialization is a follow-up); only its type
                // is needed for path-resolution type checking.
                let const_def_id = match def_map.modules[module_id]
                    .scope
                    .values
                    .get(&item.name)
                {
                    Some((id, _, _)) => ConstDefId::from_raw(id.to_raw()),
                    None => {
                        let id = *next_local_def_id;
                        *next_local_def_id += 1;
                        ConstDefId::from_raw(id)
                    }
                };
                let empty_params: HashMap<Name, Ty> = HashMap::new();
                let const_ty = tyconv::resolve_type_ref(
                    ctx,
                    infer,
                    def_map,
                    diagnostics,
                    &c.ty,
                    &empty_params,
                    item_span,
                );
                ctx.register_const_ty(const_def_id, const_ty);

                // Part C: const value materialization. Evaluate the constant's
                // initializer body (lowered in HIR) to a `ConstValue` and store
                // it so MIR lowering can fold `ConstRef` into a concrete
                // `MirConstKind`. Evaluation failures are surfaced as
                // diagnostics rather than silently producing a wrong value.
                if let (Some(body_id), Some(root_expr)) = (c.body, c.root_expr) {
                    let body = &hir.bodies[body_id];
                    let mut evaluator = glyim_const_eval::ConstEvaluator::new(body)
                        .with_interner(ctx.resolver());
                    match evaluator.evaluate(root_expr) {
                        Ok(value) => {
                            all_const_values.insert(const_def_id, value);
                        }
                        Err(e) => {
                            diagnostics.push(e.into_diagnostic());
                        }
                    }
                }
            }
            _ => {}
        }
    }
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
    trait_ctx: &TraitContext,
    expr_types: &mut HashMap<LocalDefId, HashMap<ExprId, Ty>>,
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
        trait_ctx,
        capture_log: Vec::new(),
    };

    let (thir_body, body_expr_types) = fn_ctxt.check(params);
    thir_bodies.push((local_def_id, thir_body));
    // Hoist the per-expression type cache out of the (now-dropped) FnCtxt so
    // typeck_crate can resolve inference variables once obligation
    // fulfillment is complete.
    expr_types.insert(local_def_id, body_expr_types);
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
            let trait_def_id = match tyconv::resolve_path_to_trait_def_id(def_map, trait_path, bound.span)
            {
                Some(id) => Some(id),
                None => {
                    let path_str = trait_path
                        .segments
                        .iter()
                        .map(|s| ctx.name_str(s.name))
                        .collect::<Vec<_>>()
                        .join("::");
                    diagnostics.push(GlyimDiagnostic::type_error(
                        bound.span,
                        format!("unresolved trait `{}` in where clause", path_str),
                    ));
                    None
                }
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
    /// Resolved type of a HIR expression.
    ///
    /// Returns `None` if `body_id`/`expr_id` was not checked (e.g. the body
    /// had no `Fn` item, or the id is out of range). The type is fully
    /// resolved (no inference variables) — see `typeck_crate`'s post-
    /// fulfillment resolution step.
    pub fn expr_ty(&self, body_id: LocalDefId, expr_id: usize) -> Option<Ty> {
        self.expr_types
            .get(&body_id)
            .and_then(|m| m.get(&ExprId::from_raw(expr_id as u32)))
            .copied()
    }
    /// Resolved type of a HIR pattern.
    ///
    /// Patterns are not yet collected into a per-`PatId` cache during
    /// `typeck_crate`, so this returns `None` rather than a fake value.
    /// Wiring pattern-type collection (mirroring `expr_types`) is a follow-up.
    pub fn pat_ty(&self, _body_id: LocalDefId, _pat_id: usize) -> Option<Ty> {
        None
    }
    pub fn adjustments(&self, _body_id: LocalDefId, _expr_id: usize) -> &[Adjustment] {
        &[]
    }
}