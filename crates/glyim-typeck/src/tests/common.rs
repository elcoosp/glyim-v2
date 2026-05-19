//! Common utilities for typeck tests.

use glyim_core::def_id::DefId;
use glyim_core::interner::Name;
use glyim_hir::*;
use glyim_solve::{InferenceTable, Obligation};
use glyim_span::Span;
use glyim_type::{Ty, TyCtxMut};
use std::collections::HashMap;

use crate::check_body::FnCtxt;
use crate::env::LocalEnv;
use crate::thir;

/// Test helper: constructs FnCtxt and runs the check.
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
