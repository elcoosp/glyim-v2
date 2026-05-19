//! Common utilities for typeck tests.

use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::interner::Name;
use glyim_hir::{Body, BodyId, CrateHir, Expr, ExprId};
use glyim_span::Span;
use glyim_type::Ty;

/// Create a Name from a string.
pub fn name(s: &str) -> Name {
    use glyim_core::interner::Interner;
    let mut interner = Interner::new();
    interner.intern(s)
}

/// Build a minimal HIR with a single body from a list of expressions.
pub fn make_single_body_hir(exprs: Vec<Expr>) -> (CrateHir, BodyId) {
    let mut hir = CrateHir::default();
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
pub fn typeck_single_body(_hir: &CrateHir, _body_id: BodyId) -> crate::thir::Body {
    crate::thir::Body {
        owner: DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
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
        def_id: LocalDefId::from_raw(0),
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
