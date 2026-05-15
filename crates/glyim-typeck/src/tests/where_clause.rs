use glyim_core::def_id::LocalDefId;
use glyim_core::interner::{Interner, Name};
use glyim_core::primitives::{BinOp, IntTy, Mutability, Safety, Abi};
use glyim_diag::GlyimDiagnostic;
use glyim_hir::*;
use glyim_solve::{InferenceTable, Obligation, ObligationCauseCode, TraitSolver, SolverResult, TraitPredicate, Predicate};
use glyim_span::Span;
use glyim_type::*;
use glyim_typeck::*;
use glyim_typeck::thir::Body as ThirBody;
use glyim_test::{assert_has_errors, assert_no_errors, assert_diag_contains};

// Helper to create a simple HIR function with optional where predicates
fn build_test_item(inter: &mut Interner, params: Vec<Param>, ret_ty: Option<TypeRef>, body_exprs: Vec<Expr>) -> (CrateHir, BodyId) {
    let mut hir = CrateHir {
        items: IndexVec::new(),
        bodies: IndexVec::new(),
        body_owners: IndexVec::new(),
    };

    let body = Body {
        owner: LocalDefId::from_raw(0),
        exprs: {
            let mut exprs = IndexVec::new();
            for e in body_exprs {
                exprs.push(e);
            }
            exprs
        },
        pats: IndexVec::new(),
        params: Vec::new(),
        span: Span::DUMMY,
    };
    let body_id = BodyId::from_raw(hir.bodies.push(body));

    let fn_item = Item {
        id: ItemId::from_raw(0),
        name: inter.intern("test_fn"),
        kind: ItemKind::Fn(FnItem {
            params: params.clone(),
            return_ty: ret_ty,
            body: Some(body_id),
            is_unsafe: false,
            is_async: false,
            generic_params: Vec::new(),
        }),
        visibility: Visibility::Public,
        span: Span::DUMMY,
    };
    hir.items.push(fn_item);

    // Link body owner
    let mut owners = IndexVec::new();
    owners.push(LocalDefId::from_raw(0));
    hir.body_owners = owners;

    (hir, body_id)
}

// Dummy solver that approves everything
struct ApproveSolver;
impl TraitSolver for ApproveSolver {
    fn can_prove(&mut self, _ctx: &TyCtx, _pred: &TraitPredicate) -> SolverResult {
        SolverResult::Proven
    }
    fn evaluate_predicate(&mut self, _ctx: &TyCtx, _pred: &Predicate) -> SolverResult {
        SolverResult::Proven
    }
}

// Test V02-T01: Function with where T: Clone compiles if T implements Clone (run-pass)
#[test]
fn test_fn_where_clone_compiles() {
    let mut inter = Interner::new();
    let name_clone = inter.intern("Clone");
    let name_t = inter.intern("T");

    // Build simple function: fn foo<T>(x: T) -> T where T: Clone { x }
    let t_ty = TypeRef::Path(Path::from_single(name_t));
    let param = Param { name: name_t, ty: Some(t_ty.clone()), span: Span::DUMMY };
    let ret_ty = Some(t_ty.clone());

    let expr_path = Expr::Path(Path::from_single(name_t));
    let body_exprs = vec![expr_path];

    let (hir, body_id) = build_test_item(&mut inter, vec![param], ret_ty, body_exprs);

    let mut ctx = TyCtxMut::new(inter.clone());
    let mut infer = InferenceTable::new();
    let mut diagnostics = Vec::new();
    let mut pending = Vec::new();
    let mut solver = ApproveSolver;

    // Build where clause predicate: T: Clone
    // We'll call our extended function (to be implemented)
    // For now, we'll skip calling it; test will fail later.
    // We'll just ensure the test compiles.
    // The real test will invoke typeck_function_with_where.
    // Placeholder: call standard typeck and expect no errors.
    // (After implementation, this should pass)
    let result = crate::typeck_crate(ctx, &def_map, &hir, &mut solver); // dummy call, will fail until we have extended version
    // TODO: after implementation, replace above with call that includes where clauses.
    // This test will currently panic or produce errors; we'll fix later.
    assert_no_errors(&result.1.diagnostics);
}

// Test V02-T06: Missing supertrait impl -> error (compile-fail)
#[test]
fn test_missing_supertrait_errors() {
    // Similar setup: trait inherits, impl satisfies main but not supertrait.
    // This test will be fleshed out after implementation.
}

// Additional stub tests to ensure structure compiles
#[test]
fn test_stub() {
    // placeholder
}
