use glyim_core::interner::Interner;
use crate::tests::test_utils::global_interner;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, LocalDefId};
use glyim_core::primitives::IntTy;
use glyim_core::Visibility;
use glyim_def_map::*;
use glyim_hir::where_clause::*;
use glyim_hir::*;
use glyim_solve::{SolverResult, TraitSolver};
use glyim_span::Span;
use glyim_test::{assert_diag_contains, assert_has_errors, assert_no_errors};
use glyim_type::*;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn dummy_span() -> Span {
    Span::DUMMY
}

/// Build a minimal empty CrateDefMap for tests that don't need name resolution
fn empty_def_map() -> CrateDefMap {
    let inter = global_interner();
    let modules = IndexVec::new();
    CrateDefMap {
        root: ModuleId::from_raw(0),
        modules,
        krate: CrateId::from_raw(0),
        interner: inter,
    }
}

/// Build a minimal CrateHir with one function item.
fn build_simple_hir(
    inter: &mut Interner,
    generic_params: Vec<GenericParam>,
    params: Vec<Param>,
    ret_ty: Option<TypeRef>,
    body_exprs: Vec<Expr>,
    where_clauses: Vec<WhereClause>,
) -> (CrateHir, BodyId) {
    let mut hir = CrateHir {
        items: IndexVec::new(),
        bodies: IndexVec::new(),
        body_owners: IndexVec::new(),
    };
    let mut exprs = IndexVec::new();
    for e in body_exprs {
        exprs.push(e);
    }
    let body = Body {
        owner: LocalDefId::from_raw(0),
        exprs: exprs.clone(),
        pats: IndexVec::new(),
        params: Vec::new(),
        span: dummy_span(),
        expr_spans: IndexVec::from_raw(vec![Span::DUMMY; exprs.len()]),
    };
    let body_id = hir.bodies.push(body);

    let item = Item {
        id: ItemId::from_raw(0),
        name: inter.intern("test_fn"),
        kind: ItemKind::Fn(FnItem {
            params: params.clone(),
            return_ty: ret_ty,
            body: Some(body_id),
            is_unsafe: false,
            is_async: false,
            generic_params,
            where_clauses,
        }),
        visibility: Visibility::Public,
        span: dummy_span(),
    };
    hir.items.push(item);

    let mut owners = IndexVec::new();
    owners.push(LocalDefId::from_raw(0));
    hir.body_owners = owners;

    (hir, body_id)
}

/// Approve‑all solver
struct ApproveSolver;
impl TraitSolver for ApproveSolver {
    fn can_prove(&mut self, _ctx: &TyCtx, _pred: &TraitPredicate) -> SolverResult {
        SolverResult::Proven
    }
    fn evaluate_predicate(&mut self, _ctx: &TyCtx, _pred: &Predicate) -> SolverResult {
        SolverResult::Proven
    }
}

/// Solver that rejects everything
struct RejectSolver;
impl TraitSolver for RejectSolver {
    fn can_prove(&mut self, _ctx: &TyCtx, _pred: &TraitPredicate) -> SolverResult {
        SolverResult::DefiniteNo
    }
    fn evaluate_predicate(&mut self, _ctx: &TyCtx, _pred: &Predicate) -> SolverResult {
        SolverResult::DefiniteNo
    }
}

// Helper to create a generic type param `T` at index 0
fn ty_param(inter: &mut Interner, name: &str) -> GenericParam {
    GenericParam {
        name: inter.intern(name),
        kind: GenericParamKind::Type { default: None },
        span: dummy_span(),
    }
}

// ---------------------------------------------------------------------------
// V02-T01: Function with `where T: Clone` → compiles if T implements Clone
// ---------------------------------------------------------------------------
#[test]
fn t01_fn_where_clone_satisfied() {
    let mut inter = global_interner();
    let name_clone = inter.intern("Clone");
    let name_t = inter.intern("T");

    let generic_params = vec![ty_param(&mut inter, "T")];
    let t_ty = TypeRef::Path(Path::from_single(name_t));
    let param = Param {
        name: name_t,
        ty: Some(t_ty.clone()),
        span: dummy_span(),
    };
    let body_exprs = vec![Expr::Path(Path::from_single(name_t))];

    let wc = WhereClause {
        ty: t_ty.clone(),
        bounds: vec![TraitBound {
            trait_path: Path::from_single(name_clone),
            span: dummy_span(),
        }],
        span: dummy_span(),
    };

    let (hir, _body_id) = build_simple_hir(
        &mut inter,
        generic_params,
        vec![param],
        Some(t_ty),
        body_exprs,
        vec![wc],
    );

    let ctx = TyCtxMut::new(inter.clone());
    let mut solver = ApproveSolver;
    let def_map = empty_def_map();
    let (_tcx, result) = crate::typeck_crate(ctx, &def_map, &hir, &mut solver);
    assert_no_errors(&result.diagnostics);
}

// ---------------------------------------------------------------------------
// V02-T02: Trait with supertrait; impl must satisfy both → compiles
// ---------------------------------------------------------------------------
#[test]
fn t02_supertrait_impl_satisfies_both() {
    let mut inter = global_interner();
    let name_clone = inter.intern("Clone");
    let name_copy = inter.intern("Copy");
    let name_t = inter.intern("T");

    let generic_params = vec![ty_param(&mut inter, "T")];
    let t_ty = TypeRef::Path(Path::from_single(name_t));
    let param = Param {
        name: name_t,
        ty: Some(t_ty.clone()),
        span: dummy_span(),
    };
    let body_exprs = vec![Expr::Path(Path::from_single(name_t))];

    let wc = WhereClause {
        ty: t_ty.clone(),
        bounds: vec![
            TraitBound {
                trait_path: Path::from_single(name_copy),
                span: dummy_span(),
            },
            TraitBound {
                trait_path: Path::from_single(name_clone),
                span: dummy_span(),
            },
        ],
        span: dummy_span(),
    };

    let (hir, _) = build_simple_hir(
        &mut inter,
        generic_params,
        vec![param],
        Some(t_ty),
        body_exprs,
        vec![wc],
    );

    let ctx = TyCtxMut::new(inter.clone());
    let mut solver = ApproveSolver;
    let def_map = empty_def_map();
    let (_tcx, result) = crate::typeck_crate(ctx, &def_map, &hir, &mut solver);
    assert_no_errors(&result.diagnostics);
}

// ---------------------------------------------------------------------------
// V02-T04: Multiple where bounds
// ---------------------------------------------------------------------------
#[test]
fn t04_multiple_where_bounds() {
    let mut inter = global_interner();
    let name_clone = inter.intern("Clone");
    let name_debug = inter.intern("Debug");
    let name_t = inter.intern("T");

    let generic_params = vec![ty_param(&mut inter, "T")];
    let t_ty = TypeRef::Path(Path::from_single(name_t));
    let param = Param {
        name: name_t,
        ty: Some(t_ty.clone()),
        span: dummy_span(),
    };
    let body_exprs = vec![Expr::Path(Path::from_single(name_t))];

    let wc = WhereClause {
        ty: t_ty.clone(),
        bounds: vec![
            TraitBound {
                trait_path: Path::from_single(name_clone),
                span: dummy_span(),
            },
            TraitBound {
                trait_path: Path::from_single(name_debug),
                span: dummy_span(),
            },
        ],
        span: dummy_span(),
    };

    let (hir, _) = build_simple_hir(
        &mut inter,
        generic_params,
        vec![param],
        Some(t_ty),
        body_exprs,
        vec![wc],
    );

    let ctx = TyCtxMut::new(inter.clone());
    let mut solver = ApproveSolver;
    let def_map = empty_def_map();
    let (_tcx, result) = crate::typeck_crate(ctx, &def_map, &hir, &mut solver);
    assert_no_errors(&result.diagnostics);
}

// ---------------------------------------------------------------------------
// V02-T06: Missing supertrait impl → error
// ---------------------------------------------------------------------------
#[test]
fn t06_missing_supertrait_error() {
    let mut inter = global_interner();
    let name_clone = inter.intern("Clone");
    let name_t = inter.intern("T");

    let generic_params = vec![ty_param(&mut inter, "T")];
    let t_ty = TypeRef::Path(Path::from_single(name_t));
    let param = Param {
        name: name_t,
        ty: Some(t_ty.clone()),
        span: dummy_span(),
    };
    let body_exprs = vec![Expr::Path(Path::from_single(name_t))];

    let wc = WhereClause {
        ty: t_ty.clone(),
        bounds: vec![TraitBound {
            trait_path: Path::from_single(name_clone),
            span: dummy_span(),
        }],
        span: dummy_span(),
    };

    let (hir, _) = build_simple_hir(
        &mut inter,
        generic_params,
        vec![param],
        Some(t_ty),
        body_exprs,
        vec![wc],
    );

    let ctx = TyCtxMut::new(inter.clone());
    let mut solver = RejectSolver;
    let def_map = empty_def_map();
    let (_tcx, result) = crate::typeck_crate(ctx, &def_map, &hir, &mut solver);
    assert_has_errors(&result.diagnostics);
}

// ---------------------------------------------------------------------------
// V02-T07: Where bound not satisfied → error
// ---------------------------------------------------------------------------
#[test]
fn t07_where_bound_not_satisfied_error() {
    let mut inter = global_interner();
    let name_copy = inter.intern("Copy");
    let name_t = inter.intern("T");

    let generic_params = vec![ty_param(&mut inter, "T")];
    let t_ty = TypeRef::Path(Path::from_single(name_t));
    let param = Param {
        name: name_t,
        ty: Some(t_ty.clone()),
        span: dummy_span(),
    };
    let body_exprs = vec![Expr::Literal(Literal::Int(1, Some(IntTy::I32)))];

    let wc = WhereClause {
        ty: t_ty.clone(),
        bounds: vec![TraitBound {
            trait_path: Path::from_single(name_copy),
            span: dummy_span(),
        }],
        span: dummy_span(),
    };

    let (hir, _) = build_simple_hir(
        &mut inter,
        generic_params,
        vec![param],
        Some(t_ty),
        body_exprs,
        vec![wc],
    );

    let ctx = TyCtxMut::new(inter.clone());
    let mut solver = RejectSolver;
    let def_map = empty_def_map();
    let (_tcx, result) = crate::typeck_crate(ctx, &def_map, &hir, &mut solver);
    assert_has_errors(&result.diagnostics);
    assert_diag_contains(&result.diagnostics, "trait bound");
}
