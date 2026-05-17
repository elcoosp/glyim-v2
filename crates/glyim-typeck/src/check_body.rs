//! Per-function type-checking engine.

use std::collections::HashMap;

use glyim_core::def_id::DefId;
use glyim_diag::GlyimDiagnostic;
use glyim_hir::*;
use glyim_solve::{InferenceTable, Obligation};
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
