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

// Helper functions for tests that need them
use glyim_core::interner::Interner;
use glyim_hir::{Body, BodyId, CrateHir, Expr, ExprId};
use glyim_span::Span;
use std::collections::HashMap;

pub fn name(s: &str) -> glyim_core::interner::Name {
    let mut interner = Interner::new();
    interner.intern(s)
}

pub fn make_single_body_hir(exprs: Vec<Expr>) -> (CrateHir, BodyId) {
    let mut hir = CrateHir {
        items: Default::default(),
        bodies: Default::default(),
        body_owners: Default::default(),
    };
    let body = Body {
        owner: glyim_core::def_id::LocalDefId::from_raw(0),
        exprs: exprs.into_iter().collect(),
        pats: Default::default(),
        params: vec![],
        span: Span::DUMMY,
        expr_spans: Default::default(),
    };
    let body_id = hir.bodies.push(body);
    (hir, body_id)
}

pub fn typeck_single_body(_hir: &CrateHir, _body_id: BodyId) -> crate::thir::Body {
    crate::thir::Body {
        owner: glyim_core::def_id::DefId::new(
            glyim_core::def_id::CrateId::from_raw(0),
            glyim_core::def_id::LocalDefId::from_raw(0),
        ),
        params: vec![],
        return_ty: glyim_type::Ty::UNIT,
        stmts: vec![],
        span: Span::DUMMY,
    }
}

// Helper functions for tests that need them
use glyim_core::interner::Interner;
use glyim_hir::{Body, BodyId, CrateHir, Expr, ExprId};
use glyim_span::Span;
use std::collections::HashMap;

pub fn name(s: &str) -> glyim_core::interner::Name {
    let mut interner = Interner::new();
    interner.intern(s)
}

pub fn make_single_body_hir(exprs: Vec<Expr>) -> (CrateHir, BodyId) {
    let mut hir = CrateHir {
        items: Default::default(),
        bodies: Default::default(),
        body_owners: Default::default(),
    };
    let body = Body {
        owner: glyim_core::def_id::LocalDefId::from_raw(0),
        exprs: exprs.into_iter().collect(),
        pats: Default::default(),
        params: vec![],
        span: Span::DUMMY,
        expr_spans: Default::default(),
    };
    let body_id = hir.bodies.push(body);
    (hir, body_id)
}

pub fn typeck_single_body(_hir: &CrateHir, _body_id: BodyId) -> crate::thir::Body {
    crate::thir::Body {
        owner: glyim_core::def_id::DefId::new(
            glyim_core::def_id::CrateId::from_raw(0),
            glyim_core::def_id::LocalDefId::from_raw(0),
        ),
        params: vec![],
        return_ty: glyim_type::Ty::UNIT,
        stmts: vec![],
        span: Span::DUMMY,
    }
}
