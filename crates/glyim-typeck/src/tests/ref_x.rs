use super::test_utils::{empty_def_map, make_ty_ctx};
use crate::typeck_crate;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::interner::Interner;
use glyim_core::primitives::Visibility;
use glyim_core::primitives::*;
use glyim_hir::{Body, BodyId, CrateHir, Expr, ExprId, FnItem, Item, ItemId, ItemKind, Pat, PatId};
use glyim_span::Span;
use glyim_test::{assert_no_errors, mock::MockSolver};

#[test]
fn ref_immutable() {
    let inter = Interner::new();
    let main_name = inter.intern("main");
    let x_name = inter.intern("x");

    let mut pats: IndexVec<PatId, Pat> = IndexVec::new();
    let x_pat = pats.push(Pat::Binding {
        name: x_name,
        mutability: Mutability::Not,
        subpattern: None,
    });

    let mut exprs: IndexVec<ExprId, Expr> = IndexVec::new();
    let x_expr = exprs.push(Expr::Path(glyim_hir::Path::from_single(x_name)));
    exprs.push(Expr::Ref {
        expr: x_expr,
        mutability: Mutability::Not,
    });
    exprs.push(Expr::Literal(glyim_hir::Literal::Unit));

    let body = Body {
        owner: LocalDefId::from_raw(0),
        exprs,
        pats,
        params: vec![x_pat],
        span: Span::DUMMY,
        expr_spans: IndexVec::new(),
    };
    let mut bodies: IndexVec<BodyId, Body> = IndexVec::new();
    let body_id = bodies.push(body);

    let param = glyim_hir::Param {
        name: x_name,
        ty: Some(glyim_hir::TypeRef::Path(glyim_hir::Path::from_single(
            inter.intern("i32"),
        ))),
        span: Span::DUMMY,
    };

    let item = Item {
        id: ItemId::from_raw(0),
        name: main_name,
        kind: ItemKind::Fn(FnItem {
            params: vec![param],
            return_ty: None,
            body: Some(body_id),
            is_unsafe: false,
            is_async: false,
            generic_params: vec![],
            where_clauses: Vec::new(),
        }),
        visibility: Visibility::Public,
        span: Span::DUMMY,
    };

    let mut items: IndexVec<ItemId, Item> = IndexVec::new();
    items.push(item);
    let mut body_owners = IndexVec::new();
    body_owners.push(LocalDefId::from_raw(0));

    let hir = CrateHir {
        items,
        bodies,
        body_owners,
    };

    let ctx = make_ty_ctx();
    let def_map = empty_def_map();
    let mut solver = MockSolver::new().respond_for_any(glyim_solve::SolverResult::Proven);
    let (_, result) = typeck_crate(ctx, &def_map, &hir, &mut solver);
    assert_no_errors(&result.diagnostics);
}
