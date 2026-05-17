//! Per-function type-checking engine.

use std::collections::HashMap;

use glyim_core::def_id::DefId;
use glyim_core::interner::Name;
use glyim_diag::GlyimDiagnostic;
use glyim_hir::*;
use glyim_solve::{InferenceTable, Obligation};
use glyim_span::Span;
use glyim_type::{Ty, TyCtxMut};

use crate::env::LocalEnv;
use crate::thir;

#[allow(dead_code)]
pub struct FnCtxt<'a> {
    pub ctx: &'a mut TyCtxMut,
    pub infer: &'a mut InferenceTable,
    pub diagnostics: &'a mut Vec<GlyimDiagnostic>,
    pub pending_obligations: &'a mut Vec<Obligation>,
    pub hir: &'a CrateHir,
    pub body: &'a Body,
    pub env: LocalEnv,
    pub return_ty: Ty,
    pub owner: DefId,
    pub expr_cache: HashMap<ExprId, (thir::Expr, Ty)>,
    pub def_map: &'a glyim_def_map::CrateDefMap,
}

pub fn check_function_body(
    ctx: &mut TyCtxMut,
    infer: &mut InferenceTable,
    def_map: &glyim_def_map::CrateDefMap,
    hir: &CrateHir,
    body_id: BodyId,
    owner: DefId,
    return_ty: Ty,
    params: &[(Name, Ty, Span)],
) -> (thir::Body, Vec<GlyimDiagnostic>) {
    let body = &hir.bodies[body_id];
    let mut diagnostics = Vec::new();
    let mut obligations = Vec::new();
    let env = LocalEnv::new();

    let fn_ctxt = FnCtxt {
        ctx,
        infer,
        diagnostics: &mut diagnostics,
        pending_obligations: &mut obligations,
        hir,
        body,
        env,
        return_ty,
        owner,
        expr_cache: HashMap::new(),
        def_map,
    };

    let thir_body = fn_ctxt.check(params);
    (thir_body, diagnostics)
}
