use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::interner::{Interner, Name};
use glyim_core::primitives::{BinOp, IntTy, Mutability, Safety, Abi, UintTy};
use glyim_diag::GlyimDiagnostic;
use glyim_hir::*;
use glyim_hir::where_clause::*;
use glyim_solve::{InferenceTable, Obligation, ObligationCauseCode, TraitSolver, SolverResult, TraitPredicate, Predicate, TraitContext};
use glyim_span::Span;
use glyim_type::*;
use glyim_typeck::*;
use glyim_typeck::thir::Body as ThirBody;
use glyim_test::{assert_has_errors, assert_no_errors, assert_diag_contains};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn dummy_span() -> Span { Span::DUMMY }

/// Build a minimal CrateHir with one function item.
fn build_simple_hir(
    inter: &mut Interner,
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

    let body = Body {
        owner: LocalDefId::from_raw(0),
        exprs: {
            let mut exprs = IndexVec::new();
            for e in body_exprs { exprs.push(e); }
            exprs
        },
        pats: IndexVec::new(),
        params: Vec::new(),
        span: dummy_span(),
    };
    let body_id = BodyId::from_raw(hir.bodies.push(body));

    let item = Item {
        id: ItemId::from_raw(0),
        name: inter.intern("test_fn"),
        kind: ItemKind::Fn(FnItem {
            params: params.clone(),
            return_ty: ret_ty,
            body: Some(body_id),
            is_unsafe: false,
            is_async: false,
            generic_params: Vec::new(),
            where_clauses,
        }),
        visibility: Visibility::Public,
        span: dummy_span(),
    };
    hir.items.push(item);

    // body owners
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

// ---------------------------------------------------------------------------
// V02-T01: Function with `where T: Clone` → compiles if T implements Clone
// ---------------------------------------------------------------------------
#[test]
fn t01_fn_where_clone_satisfied() {
    let mut inter = Interner::new();
    let name_clone = inter.intern("Clone");
    let name_t = inter.intern("T");

    let t_ty = TypeRef::Path(Path::from_single(name_t));
    let param = Param { name: name_t, ty: Some(t_ty.clone()), span: dummy_span() };
    let body_exprs = vec![
        Expr::Path(Path::from_single(name_t)), // just return T
    ];

    // where T: Clone
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
        vec![param],
        Some(t_ty),
        body_exprs,
        vec![wc],
    );

    let mut ctx = TyCtxMut::new(inter.clone());
    let mut solver = ApproveSolver;
    let (_tcx, result) = typeck_crate(ctx, &glyim_def_map::CrateDefMap::empty_for_test(), &hir, &mut solver);
    assert_no_errors(&result.diagnostics);
}

// ---------------------------------------------------------------------------
// V02-T02: Trait with supertrait; impl must satisfy both → compiles
// ---------------------------------------------------------------------------
#[test]
fn t02_supertrait_impl_satisfies_both() {
    // This test will rely on where clause processing of supertrait bounds.
    // For now, we simulate a function with a where clause that requires the supertrait
    // and a concrete type known to satisfy both. Using ApproveSolver it passes.
    let mut inter = Interner::new();
    let name_clone = inter.intern("Clone");
    let name_copy = inter.intern("Copy");
    let name_t = inter.intern("T");

    let t_ty = TypeRef::Path(Path::from_single(name_t));
    let param = Param { name: name_t, ty: Some(t_ty.clone()), span: dummy_span() };
    let body_exprs = vec![Expr::Path(Path::from_single(name_t))];

    // where T: Copy (super) and T: Clone (implied by Copy)
    let wc = WhereClause {
        ty: t_ty.clone(),
        bounds: vec![
            TraitBound { trait_path: Path::from_single(name_copy), span: dummy_span() },
            TraitBound { trait_path: Path::from_single(name_clone), span: dummy_span() },
        ],
        span: dummy_span(),
    };

    let (hir, _) = build_simple_hir(
        &mut inter,
        vec![param],
        Some(t_ty),
        body_exprs,
        vec![wc],
    );

    let mut ctx = TyCtxMut::new(inter.clone());
    let mut solver = ApproveSolver;
    let (_tcx, result) = typeck_crate(ctx, &glyim_def_map::CrateDefMap::empty_for_test(), &hir, &mut solver);
    assert_no_errors(&result.diagnostics);
}

// ---------------------------------------------------------------------------
// V02-T04: Multiple where bounds
// ---------------------------------------------------------------------------
#[test]
fn t04_multiple_where_bounds() {
    let mut inter = Interner::new();
    let name_clone = inter.intern("Clone");
    let name_debug = inter.intern("Debug");
    let name_t = inter.intern("T");

    let t_ty = TypeRef::Path(Path::from_single(name_t));
    let param = Param { name: name_t, ty: Some(t_ty.clone()), span: dummy_span() };
    let body_exprs = vec![Expr::Path(Path::from_single(name_t))];

    let wc = WhereClause {
        ty: t_ty.clone(),
        bounds: vec![
            TraitBound { trait_path: Path::from_single(name_clone), span: dummy_span() },
            TraitBound { trait_path: Path::from_single(name_debug), span: dummy_span() },
        ],
        span: dummy_span(),
    };

    let (hir, _) = build_simple_hir(
        &mut inter,
        vec![param],
        Some(t_ty),
        body_exprs,
        vec![wc],
    );

    let mut ctx = TyCtxMut::new(inter.clone());
    let mut solver = ApproveSolver;
    let (_tcx, result) = typeck_crate(ctx, &glyim_def_map::CrateDefMap::empty_for_test(), &hir, &mut solver);
    assert_no_errors(&result.diagnostics);
}

// ---------------------------------------------------------------------------
// V02-T06: Missing supertrait impl → error
// ---------------------------------------------------------------------------
#[test]
fn t06_missing_supertrait_error() {
    // We'll create a where clause requiring `T: Clone`, but the solver will reject it.
    let mut inter = Interner::new();
    let name_clone = inter.intern("Clone");
    let name_t = inter.intern("T");

    let t_ty = TypeRef::Path(Path::from_single(name_t));
    let param = Param { name: name_t, ty: Some(t_ty.clone()), span: dummy_span() };
    let body_exprs = vec![Expr::Path(Path::from_single(name_t))];

    let wc = WhereClause {
        ty: t_ty.clone(),
        bounds: vec![TraitBound { trait_path: Path::from_single(name_clone), span: dummy_span() }],
        span: dummy_span(),
    };

    let (hir, _) = build_simple_hir(
        &mut inter,
        vec![param],
        Some(t_ty),
        body_exprs,
        vec![wc],
    );

    let mut ctx = TyCtxMut::new(inter.clone());
    let mut solver = RejectSolver;
    let (_tcx, result) = typeck_crate(ctx, &glyim_def_map::CrateDefMap::empty_for_test(), &hir, &mut solver);
    assert_has_errors(&result.diagnostics);
}

// ---------------------------------------------------------------------------
// V02-T07: Where bound not satisfied → error
// ---------------------------------------------------------------------------
#[test]
fn t07_where_bound_not_satisfied_error() {
    let mut inter = Interner::new();
    let name_copy = inter.intern("Copy");
    let name_t = inter.intern("T");

    let t_ty = TypeRef::Path(Path::from_single(name_t));
    let param = Param { name: name_t, ty: Some(t_ty.clone()), span: dummy_span() };
    let body_exprs = vec![Expr::Literal(Literal::Int(1, Some(IntTy::I32)))]; // need something

    let wc = WhereClause {
        ty: t_ty.clone(),
        bounds: vec![TraitBound { trait_path: Path::from_single(name_copy), span: dummy_span() }],
        span: dummy_span(),
    };

    let (hir, _) = build_simple_hir(
        &mut inter,
        vec![param],
        Some(t_ty),
        body_exprs,
        vec![wc],
    );

    let mut ctx = TyCtxMut::new(inter.clone());
    let mut solver = RejectSolver;
    let (_tcx, result) = typeck_crate(ctx, &glyim_def_map::CrateDefMap::empty_for_test(), &hir, &mut solver);
    assert_has_errors(&result.diagnostics);
    assert_diag_contains(&result.diagnostics, "trait bound");
}
