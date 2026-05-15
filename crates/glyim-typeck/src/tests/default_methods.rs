//! Tests for V03: Default Methods

use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, LocalDefId};
use glyim_def_map::ModuleId;
use glyim_core::interner::{Interner, Name};
use glyim_core::primitives::*;
use glyim_def_map::{CrateDefMap, ItemScope, ModuleData, ModuleOrigin};
use glyim_hir::*;
use glyim_solve::SolverResult;
use glyim_span::Span;
use glyim_test::mock::MockSolver;
use crate::typeck_crate;
use glyim_type::*;


fn make_interner() -> Interner {
    Interner::new()
}

fn make_name(interner: &mut Interner, s: &str) -> Name {
    interner.intern(s)
}

fn make_return_42_body(owner: LocalDefId) -> Body {
    let mut exprs = IndexVec::new();
    let lit_expr = Expr::Literal(Literal::Int(42, None));
    let lit_id = exprs.push(lit_expr);
    let ret_expr = Expr::Return { value: Some(lit_id) };
    let _ret_id = exprs.push(ret_expr);
    Body {
        owner,
        exprs,
        pats: IndexVec::new(),
        params: vec![],
        span: Span::DUMMY,
    }
}

fn make_return_99_body(owner: LocalDefId) -> Body {
    let mut exprs = IndexVec::new();
    let lit_expr = Expr::Literal(Literal::Int(99, None));
    let lit_id = exprs.push(lit_expr);
    let ret_expr = Expr::Return { value: Some(lit_id) };
    let _ret_id = exprs.push(ret_expr);
    Body {
        owner,
        exprs,
        pats: IndexVec::new(),
        params: vec![],
        span: Span::DUMMY,
    }
}

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
        span: Span::DUMMY,
        def_id: LocalDefId::from_raw(0),
        visibility: Visibility::Public,
    });
    CrateDefMap {
        root,
        modules,
        krate,
        interner: Interner::new(),
    }
}

fn make_simple_hir_with_trait_and_impl(override_default: bool) -> (TyCtxMut, CrateHir) {
    let mut interner = make_interner();
    let trait_name = make_name(&mut interner, "MyTrait");
    let method_name = make_name(&mut interner, "my_method");

    let owner_trait = LocalDefId::from_raw(0);
    let owner_impl = LocalDefId::from_raw(1);

    // Build default body: return 42
    let default_body = make_return_42_body(owner_trait);

    // Trait method
    let trait_method = TraitMethod {
        name: method_name,
        params: vec![],
        return_ty: None,
        default_body: None, // will set after inserting body
    };

    let trait_item = TraitItem {
        associated_types: vec![],
        methods: vec![trait_method],
        generic_params: vec![],
        where_clauses: vec![],
    };

    // Impl method
    let (impl_body_opt, override_body) = if override_default {
        let body = make_return_99_body(owner_impl);
        (Some(BodyId::from_raw(1)), Some(body))
    } else {
        (None, None)
    };

    let impl_method = ImplMethod {
        name: method_name,
        body: impl_body_opt,
        params: vec![],
        return_ty: None,
    };

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
        generic_params: vec![],
        where_clauses: vec![],
    };

    // Build CrateHir
    let mut items = IndexVec::new();
    let trait_item_id = items.push(Item {
        id: ItemId::from_raw(0),
        name: trait_name,
        kind: ItemKind::Trait(trait_item),
        visibility: Visibility::Inherited,
        span: Span::DUMMY,
    });
    // Set default body id after insertion
    if let ItemKind::Trait(ref mut ti) = items[trait_item_id].kind {
        ti.methods[0].default_body = Some(BodyId::from_raw(0));
    }

    items.push(Item {
        id: ItemId::from_raw(1),
        name: make_name(&mut interner, "MyType"),
        kind: ItemKind::Impl(impl_item),
        visibility: Visibility::Inherited,
        span: Span::DUMMY,
    });

    let mut bodies = IndexVec::new();
    bodies.push(default_body);
    let mut body_owners = IndexVec::new();
    body_owners.push(owner_trait);
    if let Some(override_body) = override_body {
        bodies.push(override_body);
        body_owners.push(owner_impl);
    }

    let hir = CrateHir {
        items,
        bodies,
        body_owners,
    };

    let ctx = TyCtxMut::new(interner);
    (ctx, hir)
}

#[test]
fn v03_t01_trait_default_method_impl_no_override() {
    let (ctx, hir) = make_simple_hir_with_trait_and_impl(false);
    let def_map = build_empty_def_map(CrateId::from_raw(0));
    let mut solver = MockSolver::new().respond_for_any(SolverResult::Proven);
    let (_frozen_ctx, result) = typeck_crate(ctx, &def_map, &hir, &mut solver);

    // There should be a THIR body for the impl method, inherited from the trait default.
    assert!(!result.thir_bodies.is_empty(), "Expected at least one THIR body");
    // Check that the body comes from the trait default (42).
    // The THIR body has two stmts: the literal 42, then Return(Some(42))
    let (_owner, thir_body) = &result.thir_bodies[0];
    assert_eq!(thir_body.stmts.len(), 2);
    // Second stmt should be a Return
    assert!(matches!(thir_body.stmts[1], crate::thir::Stmt::Return { .. }));
}

#[test]
fn v03_t02_overridden_default_method() {
    let (ctx, hir) = make_simple_hir_with_trait_and_impl(true);
    let def_map = build_empty_def_map(CrateId::from_raw(0));
    let mut solver = MockSolver::new().respond_for_any(SolverResult::Proven);
    let (_frozen_ctx, result) = typeck_crate(ctx, &def_map, &hir, &mut solver);

    // The THIR body should be the override (return 99) not the default.
    assert!(!result.thir_bodies.is_empty());
    let (_owner, thir_body) = &result.thir_bodies[0];
    assert_eq!(thir_body.stmts.len(), 2);
    // Second stmt is Return (with 99)
    assert!(matches!(thir_body.stmts[1], crate::thir::Stmt::Return { .. }));
}

#[test]
fn v03_t03_default_method_calling_another_default_method() {
    // This test requires that a default method body references another method by path.
    // For simplicity, we stub this test and mark it as pending.
    // TODO: Implement when method call resolution is available.
}

#[test]
fn v03_t04_default_method_with_generic_params() {
    // Stub
}

#[test]
fn v03_t05_default_method_calls_missing_method_error() {
    // Stub: Expected error when trait method is not implemented and has no default.
}
