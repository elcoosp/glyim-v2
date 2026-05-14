use crate::typeck_crate;
use glyim_core::arena::IndexVec;
use glyim_core::interner::Interner;
use glyim_core::primitives::*;
use glyim_core::def_id::LocalDefId;
use glyim_core::primitives::Visibility;
use glyim_hir::{
    Body, BodyId, CrateHir, Expr, ExprId, FnItem, Item, ItemId, ItemKind, Pat, PatId,
};
use glyim_span::Span;
use glyim_test::{assert_no_errors, mock::MockSolver};
use super::test_utils::{empty_def_map, make_ty_ctx};

#[test]
fn binary_i32_add_ok() {
    let mut inter = Interner::new();
    let main_name = inter.intern("main");

    let mut exprs: IndexVec<ExprId, Expr> = IndexVec::new();
    let lhs = exprs.push(Expr::Literal(glyim_hir::Literal::Int(1, Some(IntTy::I32))));
    let rhs = exprs.push(Expr::Literal(glyim_hir::Literal::Int(2, Some(IntTy::I32))));
    let add = exprs.push(Expr::Binary {
        op: BinOp::Add,
        lhs,
        rhs,
    });

    let body = Body {
        owner: LocalDefId::from_raw(0),
        exprs,
        pats: IndexVec::new(),
        params: vec![],
        span: Span::DUMMY,
    };
    let mut bodies: IndexVec<BodyId, Body> = IndexVec::new();
    let body_id = bodies.push(body);

    let item = Item {
        id: ItemId::from_raw(0),
        name: main_name,
        kind: ItemKind::Fn(FnItem {
            params: vec![],
            return_ty: None,
            body: Some(body_id),
            is_unsafe: false,
            is_async: false,
            generic_params: vec![],
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
