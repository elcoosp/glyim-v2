use super::test_utils::{empty_def_map, make_ty_ctx};
use crate::typeck_crate;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::LocalDefId;
use glyim_core::interner::Interner;
use glyim_core::primitives::Visibility;
use glyim_core::primitives::*;
use glyim_hir::{Body, BodyId, CrateHir, Expr, ExprId, FnItem, Item, ItemId, ItemKind, Pat, PatId};
use glyim_span::Span;
use glyim_test::{assert_has_errors, assert_no_errors, mock::MockSolver};

/// S14-T11: Multiple functions in one crate
#[test]
fn multiple_functions() {
    let inter = Interner::new();
    let fn_a_name = inter.intern("fn_a");
    let fn_b_name = inter.intern("fn_b");
    let make_body = || {
        let mut exprs: IndexVec<ExprId, Expr> = IndexVec::new();
        exprs.push(Expr::Literal(glyim_hir::Literal::Unit));
        Body {
            owner: LocalDefId::from_raw(0),
            exprs: exprs.clone(),
            pats: IndexVec::new(),
            params: vec![],
            span: Span::DUMMY,
            expr_spans: IndexVec::from_raw(vec![Span::DUMMY; exprs.clone().len()]),
        }
    };
    let mut bodies: IndexVec<BodyId, Body> = IndexVec::new();
    let body_a = bodies.push(make_body());
    let body_b = bodies.push(make_body());
    let mut items: IndexVec<ItemId, Item> = IndexVec::new();
    items.push(Item {
        id: ItemId::from_raw(0),
        name: fn_a_name,
        kind: ItemKind::Fn(FnItem {
            return_ty: None,
            body: Some(body_a),
            is_unsafe: false,
            is_async: false,
            generic_params: vec![],
            where_clauses: Vec::new(),
        }),
        visibility: Visibility::Public,
        span: Span::DUMMY,
    });
        id: ItemId::from_raw(1),
        name: fn_b_name,
            body: Some(body_b),
    let mut body_owners = IndexVec::new();
    body_owners.push(LocalDefId::from_raw(0));
    body_owners.push(LocalDefId::from_raw(1));
    let hir = CrateHir {
        items,
        bodies,
        body_owners,
    let ctx = make_ty_ctx();
    let def_map = empty_def_map();
    let mut solver = MockSolver::new().respond_for_any(glyim_solve::SolverResult::Proven);
    let (_, result) = typeck_crate(ctx, &def_map, &hir, &mut solver);
    assert_no_errors(&result.diagnostics);
    assert_eq!(result.thir_bodies.len(), 2, "two THIR bodies expected");
}
/// S14-T12: Function with multiple parameters
fn multiple_params() {
    let main_name = inter.intern("main");
    let x_name = inter.intern("x");
    let y_name = inter.intern("y");
    let mut pats: IndexVec<PatId, Pat> = IndexVec::new();
    let x_pat = pats.push(Pat::Binding {
        name: x_name,
        mutability: Mutability::Not,
        subpattern: None,
    let y_pat = pats.push(Pat::Binding {
        name: y_name,
    let mut exprs: IndexVec<ExprId, Expr> = IndexVec::new();
    let x_expr = exprs.push(Expr::Path(glyim_hir::Path::from_single(x_name)));
    let y_expr = exprs.push(Expr::Path(glyim_hir::Path::from_single(y_name)));
    exprs.push(Expr::Binary {
        op: BinOp::Add,
        lhs: x_expr,
        rhs: y_expr,
    exprs.push(Expr::Literal(glyim_hir::Literal::Unit));
    let body = Body {
        owner: LocalDefId::from_raw(0),
        exprs: exprs.clone(),
        pats,
        params: vec![x_pat, y_pat],
        expr_spans: IndexVec::from_raw(vec![Span::DUMMY; exprs.clone().len()]),
    let body_id = bodies.push(body);
    let param_x = glyim_hir::Param {
        ty: Some(glyim_hir::TypeRef::Path(glyim_hir::Path::from_single(
            inter.intern("i32"),
        ))),
    let param_y = glyim_hir::Param {
    let item = Item {
        name: main_name,
            params: vec![param_x, param_y],
            body: Some(body_id),
    items.push(item);
/// S14-T13: i32 * i32 (different operator)
fn binary_multiply() {
    let lhs = exprs.push(Expr::Literal(glyim_hir::Literal::Int(3, Some(IntTy::I32))));
    let rhs = exprs.push(Expr::Literal(glyim_hir::Literal::Int(4, Some(IntTy::I32))));
        op: BinOp::Mul,
        lhs,
        rhs,
        pats: IndexVec::new(),
        params: vec![],
/// S14-T14: Comparison operators (Eq)
fn binary_comparison() {
    let lhs = exprs.push(Expr::Literal(glyim_hir::Literal::Int(1, Some(IntTy::I32))));
    let rhs = exprs.push(Expr::Literal(glyim_hir::Literal::Int(2, Some(IntTy::I32))));
        op: BinOp::Eq,
/// S14-T15: If expression
fn if_expression() {
    let cond = exprs.push(Expr::Literal(glyim_hir::Literal::Bool(true)));
    let then_val = exprs.push(Expr::Literal(glyim_hir::Literal::Int(1, Some(IntTy::I32))));
    let else_val = exprs.push(Expr::Literal(glyim_hir::Literal::Int(2, Some(IntTy::I32))));
    exprs.push(Expr::If {
        cond,
        then_branch: then_val,
        else_branch: Some(else_val),
/// S14-T16: Block expression with statements
fn block_expression() {
    let lit1 = exprs.push(Expr::Literal(glyim_hir::Literal::Int(10, Some(IntTy::I32))));
    let lit2 = exprs.push(Expr::Literal(glyim_hir::Literal::Int(20, Some(IntTy::I32))));
        lhs: lit1,
        rhs: lit2,
/// S14-T17: Unresolved variable reference produces error
fn unresolved_variable() {
    exprs.push(Expr::Path(glyim_hir::Path::from_single(
        inter.intern("undefined"),
    )));
    assert_has_errors(&result.diagnostics);
