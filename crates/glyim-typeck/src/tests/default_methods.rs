//! Tests for V03: Default Methods

use crate::typeck_crate;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, LocalDefId};
use glyim_core::interner::{Interner, Name};
use glyim_core::primitives::*;
use glyim_def_map::{CrateDefMap, ItemScope, ModuleData, ModuleId, ModuleOrigin};
use glyim_hir::*;
use glyim_solve::SolverResult;
use glyim_span::Span;
use glyim_test::mock::MockSolver;
use glyim_type::*;
fn make_interner() -> Interner {
    Interner::new()
}
fn make_name(interner: &mut Interner, s: &str) -> Name {
    interner.intern(s)
fn make_return_42_body(owner: LocalDefId) -> Body {
    let mut exprs = IndexVec::new();
    let lit_expr = Expr::Literal(Literal::Int(42, None));
    let lit_id = exprs.push(lit_expr);
    let ret_expr = Expr::Return {
        value: Some(lit_id),
    };
    let _ret_id = exprs.push(ret_expr);
    Body {
        owner,
        exprs: exprs.clone(),
        pats: IndexVec::new(),
        params: vec![],
        span: Span::DUMMY,
        expr_spans: IndexVec::from_raw(vec![Span::DUMMY; exprs.len()]),
    }
fn make_return_99_body(owner: LocalDefId) -> Body {
    let lit_expr = Expr::Literal(Literal::Int(99, None));
        expr_spans: IndexVec::from_raw(vec![Span::DUMMY; exprs.clone().len()]),
fn make_body_with_call(owner: LocalDefId, callee_name: Name) -> Body {
    let path = Path::from_single(callee_name);
    let callee_path_id = exprs.push(Expr::Path(path));
    let call_id = exprs.push(Expr::Call {
        func: callee_path_id,
        args: vec![],
    });
    let _ret_id = exprs.push(Expr::Return {
        value: Some(call_id),
fn build_empty_def_map(krate: CrateId) -> CrateDefMap {
    let mut modules = IndexVec::new();
    let root = ModuleId::from_raw(0);
    modules.push(ModuleData {
        parent: None,
        children: Vec::new(),
        scope: ItemScope {
            types: Vec::new(),
            values: Vec::new(),
            macros: Vec::new(),
        },
        origin: ModuleOrigin::CrateRoot,
        def_id: LocalDefId::from_raw(0),
        visibility: Visibility::Public,
    CrateDefMap {
        root,
        modules,
        krate,
        interner: Interner::new(),
fn make_simple_hir_with_trait_and_impl(override_default: bool) -> (TyCtxMut, CrateHir) {
    let mut interner = make_interner();
    let trait_name = make_name(&mut interner, "MyTrait");
    let method_name = make_name(&mut interner, "my_method");
    let owner_trait = LocalDefId::from_raw(0);
    let owner_impl = LocalDefId::from_raw(1);
    let default_body = make_return_42_body(owner_trait);
    let trait_method = TraitMethod {
        name: method_name,
        return_ty: None,
        default_body: None,
    let trait_item = TraitItem {
        associated_types: vec![],
        methods: vec![trait_method],
        generic_params: vec![],
        where_clauses: vec![],
    let (impl_body_opt, override_body) = if override_default {
        let body = make_return_99_body(owner_impl);
        (Some(BodyId::from_raw(1)), Some(body))
    } else {
        (None, None)
    let impl_method = ImplMethod {
        body: impl_body_opt,
    let impl_item = ImplItem {
        trait_ref: Some(Path {
            segments: vec![PathSegment {
                name: trait_name,
                generic_args: None,
            }],
            kind: glyim_core::path::PathKind::Plain,
        }),
        self_ty: TypeRef::Path(Path::from_single(make_name(&mut interner, "MyType"))),
        methods: vec![impl_method],
    let mut items = IndexVec::new();
    let trait_item_id = items.push(Item {
        id: ItemId::from_raw(0),
        name: trait_name,
        kind: ItemKind::Trait(trait_item),
        visibility: Visibility::Inherited,
    if let ItemKind::Trait(ref mut ti) = items[trait_item_id].kind {
        ti.methods[0].default_body = Some(BodyId::from_raw(0));
    items.push(Item {
        id: ItemId::from_raw(1),
        name: make_name(&mut interner, "MyType"),
        kind: ItemKind::Impl(impl_item),
    let mut bodies = IndexVec::new();
    bodies.push(default_body);
    let mut body_owners = IndexVec::new();
    body_owners.push(owner_trait);
    if let Some(override_body) = override_body {
        bodies.push(override_body);
        body_owners.push(owner_impl);
    let hir = CrateHir {
        items,
        bodies,
        body_owners,
    let ctx = TyCtxMut::new(interner);
    (ctx, hir)
#[test]
fn v03_t01_trait_default_method_impl_no_override() {
    let (ctx, hir) = make_simple_hir_with_trait_and_impl(false);
    let def_map = build_empty_def_map(CrateId::from_raw(0));
    let mut solver = MockSolver::new().respond_for_any(SolverResult::Proven);
    let (_frozen_ctx, result) = typeck_crate(ctx, &def_map, &hir, &mut solver);
    assert!(
        !result.thir_bodies.is_empty(),
        "Expected at least one THIR body"
    );
    let (_owner, thir_body) = &result.thir_bodies[0];
    assert_eq!(thir_body.stmts.len(), 2);
    assert!(matches!(
        thir_body.stmts[1],
        crate::thir::Stmt::Return { .. }
    ));
fn v03_t02_overridden_default_method() {
    let (ctx, hir) = make_simple_hir_with_trait_and_impl(true);
    assert!(!result.thir_bodies.is_empty());
fn v03_t03_default_method_calling_another_default_method() {
    // Setup: trait MyTrait { fn bar() -> i32 { 42 }  fn foo() -> i32 { bar() } }
    // Impl MyType for MyTrait {} (inherits both)
    let bar_name = make_name(&mut interner, "bar");
    let foo_name = make_name(&mut interner, "foo");
    let _owner_impl = LocalDefId::from_raw(1);
    let bar_body = make_return_42_body(owner_trait);
    let foo_body = make_body_with_call(owner_trait, bar_name);
    let trait_methods = vec![
        TraitMethod {
            name: bar_name,
            params: vec![],
            return_ty: None,
            default_body: None, // set later
            name: foo_name,
    ];
        methods: trait_methods,
    let impl_methods = vec![
        ImplMethod {
            body: None,
        methods: impl_methods,
    // Assign default body ids after insertion
        ti.methods[1].default_body = Some(BodyId::from_raw(1));
    bodies.push(bar_body);
    bodies.push(foo_body);
    let body_owners = IndexVec::from_raw(vec![owner_trait, owner_trait]);
    // Should have two THIR bodies (bar and foo)
    assert_eq!(result.thir_bodies.len(), 2, "Expected two method bodies");
    // One of them should contain a Call expression (the foo body)
    let has_call = result.thir_bodies.iter().any(|(_owner, body)| {
        body.stmts.iter().any(|stmt| {
            if let crate::thir::Stmt::Expr { expr } = stmt {
                matches!(expr.kind, crate::thir::ExprKind::Call { .. })
            } else {
                false
            }
        })
    assert!(has_call, "Expected a Call expression in one of the bodies");
fn v03_t04_default_method_with_generic_params() {
    // Setup: trait MyTrait<T> { fn my_method() -> T { ... } }
    // Impl MyType for MyTrait<i32> { } (inherits default)
    let t_name = make_name(&mut interner, "T");
    let generic_params = vec![GenericParam {
        name: t_name,
        kind: GenericParamKind::Type { default: None },
    }];
        default_body: None, // set later
        generic_params: generic_params.clone(),
        body: None,
                generic_args: Some(vec![TypeRef::Path(Path::from_single(make_name(
                    &mut interner,
                    "i32",
                )))]),
        generic_params: vec![], // impl not polymorphic
    let body_owners = IndexVec::from_raw(vec![owner_trait]);
    // Should have at least one body, no errors
    assert!(result.diagnostics.is_empty());
fn v03_t05_default_method_calls_missing_method_error() {
    // Setup: trait MyTrait { fn missing() -> i32; }
    // Impl MyType for MyTrait { } // does NOT provide missing method -> error
    let method_name = make_name(&mut interner, "missing");
        default_body: None, // no default
        body: None, // no implementation
        bodies: IndexVec::new(),
        body_owners: IndexVec::new(),
        !result.diagnostics.is_empty(),
        "Expected an error diagnostic"
    let error_msg = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("has no implementation and no default"));
        error_msg,
        "Expected missing method diagnostic, got: {:?}",
        result.diagnostics
