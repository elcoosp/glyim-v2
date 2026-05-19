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

// ========== Helpers for test files that rely on common ==========

/// Create a Name from a string.
pub fn name(s: &str) -> glyim_core::interner::Name {
    let mut interner = glyim_core::interner::Interner::new();
    interner.intern(s)
}

/// Build a minimal HIR with a single body from a list of expressions.
pub fn make_single_body_hir(exprs: Vec<glyim_hir::Expr>) -> (glyim_hir::CrateHir, glyim_hir::BodyId) {
    use glyim_core::arena::IndexVec;
    use glyim_core::def_id::LocalDefId;
    use glyim_hir::{Body, CrateHir, ExprId};
    let mut hir = CrateHir {
        items: Default::default(),
        bodies: Default::default(),
        body_owners: Default::default(),
    };
    let mut expr_vec = IndexVec::new();
    for expr in exprs {
        expr_vec.push(expr);
    }
    let expr_spans = IndexVec::from_raw(vec![Span::DUMMY; expr_vec.len()]);
    let body = Body {
        owner: LocalDefId::from_raw(0),
        exprs: expr_vec,
        pats: IndexVec::new(),
        params: vec![],
        span: Span::DUMMY,
        expr_spans,
    };
    let body_id = hir.bodies.push(body);
    (hir, body_id)
}

/// Type‑check a single body (returns a dummy THIR body for tests that only need to compile).
pub fn typeck_single_body(_hir: &glyim_hir::CrateHir, _body_id: glyim_hir::BodyId) -> crate::thir::Body {
    use glyim_core::def_id::{CrateId, LocalDefId};
    use glyim_span::Span;
    use glyim_type::Ty;
    crate::thir::Body {
        owner: glyim_core::def_id::DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        params: vec![],
        return_ty: Ty::UNIT,
        stmts: vec![],
        span: Span::DUMMY,
    }
}

/// Dummy empty def map (used by some old test files).
pub fn empty_def_map() -> glyim_def_map::CrateDefMap {
    use glyim_core::arena::IndexVec;
    use glyim_core::def_id::CrateId;
    use glyim_def_map::{CrateDefMap, ModuleData, ModuleId, ModuleOrigin, ItemScope};
    use glyim_span::{FileId, Span, SyntaxContext, ByteIdx};
    let interner = glyim_core::interner::Interner::new();
    let root = ModuleId::from_raw(0);
    let module_data = ModuleData {
        parent: None,
        children: vec![],
        scope: ItemScope::default(),
        origin: ModuleOrigin::CrateRoot,
        span: Span::new(FileId::from_raw(0), ByteIdx::from_raw(0), ByteIdx::from_raw(0), SyntaxContext::ROOT),
        def_id: glyim_core::def_id::LocalDefId::from_raw(0),
        visibility: glyim_core::primitives::Visibility::Public,
    };
    let modules = IndexVec::from_raw(vec![module_data]);
    CrateDefMap {
        root,
        modules,
        krate: CrateId::from_raw(0),
        interner,
    }
}

/// Dummy type context for tests.
pub fn make_ty_ctx() -> glyim_type::TyCtxMut {
    glyim_type::TyCtxMut::new(Default::default())
}

// ========== Helpers for test files that rely on common ==========

/// Create a Name from a string.
pub fn name(s: &str) -> glyim_core::interner::Name {
    let mut interner = glyim_core::interner::Interner::new();
    interner.intern(s)
}

/// Build a minimal HIR with a single body from a list of expressions.
pub fn make_single_body_hir(exprs: Vec<glyim_hir::Expr>) -> (glyim_hir::CrateHir, glyim_hir::BodyId) {
    use glyim_core::arena::IndexVec;
    use glyim_core::def_id::LocalDefId;
    use glyim_hir::{Body, CrateHir, ExprId};
    let mut hir = CrateHir {
        items: Default::default(),
        bodies: Default::default(),
        body_owners: Default::default(),
    };
    let mut expr_vec = IndexVec::new();
    for expr in exprs {
        expr_vec.push(expr);
    }
    let expr_spans = IndexVec::from_raw(vec![Span::DUMMY; expr_vec.len()]);
    let body = Body {
        owner: LocalDefId::from_raw(0),
        exprs: expr_vec,
        pats: IndexVec::new(),
        params: vec![],
        span: Span::DUMMY,
        expr_spans,
    };
    let body_id = hir.bodies.push(body);
    (hir, body_id)
}

/// Type‑check a single body (returns a dummy THIR body for tests that only need to compile).
pub fn typeck_single_body(_hir: &glyim_hir::CrateHir, _body_id: glyim_hir::BodyId) -> crate::thir::Body {
    use glyim_core::def_id::{CrateId, LocalDefId};
    use glyim_span::Span;
    use glyim_type::Ty;
    crate::thir::Body {
        owner: glyim_core::def_id::DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        params: vec![],
        return_ty: Ty::UNIT,
        stmts: vec![],
        span: Span::DUMMY,
    }
}

/// Dummy empty def map (used by some old test files).
pub fn empty_def_map() -> glyim_def_map::CrateDefMap {
    use glyim_core::arena::IndexVec;
    use glyim_core::def_id::CrateId;
    use glyim_def_map::{CrateDefMap, ModuleData, ModuleId, ModuleOrigin, ItemScope};
    use glyim_span::{FileId, Span, SyntaxContext, ByteIdx};
    let interner = glyim_core::interner::Interner::new();
    let root = ModuleId::from_raw(0);
    let module_data = ModuleData {
        parent: None,
        children: vec![],
        scope: ItemScope::default(),
        origin: ModuleOrigin::CrateRoot,
        span: Span::new(FileId::from_raw(0), ByteIdx::from_raw(0), ByteIdx::from_raw(0), SyntaxContext::ROOT),
        def_id: glyim_core::def_id::LocalDefId::from_raw(0),
        visibility: glyim_core::primitives::Visibility::Public,
    };
    let modules = IndexVec::from_raw(vec![module_data]);
    CrateDefMap {
        root,
        modules,
        krate: CrateId::from_raw(0),
        interner,
    }
}

/// Dummy type context for tests.
pub fn make_ty_ctx() -> glyim_type::TyCtxMut {
    glyim_type::TyCtxMut::new(Default::default())
}
