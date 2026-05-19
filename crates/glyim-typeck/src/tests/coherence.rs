//! Tests for coherence and orphan rules (Stream V04).

use glyim_core::def_id::{CrateId, LocalDefId, TraitDefId};
use glyim_core::interner::Interner;
use glyim_core::primitives::Visibility;
use glyim_def_map::{CrateDefMap, ItemScope, ModuleData, ModuleId, ModuleOrigin};
use glyim_hir::{ImplItem, Path, TypeRef};
use glyim_span::Span;
use glyim_type::{ImplPolarity, Substitution, Ty, TyCtxMut};

use crate::coherence::{CoherenceChecker, ResolvedImplHeader};
use super::common::make_ty_ctx;

// ---------------------------------------------------------------------------
// Helper: convert ImplItem to ResolvedImplHeader for testing
// ---------------------------------------------------------------------------
fn impl_item_to_header(
    impl_item: &ImplItem,
    interner: &mut Interner,
    _ctx: &mut TyCtxMut,
    _def_map: &CrateDefMap,
) -> ResolvedImplHeader {
    let trait_name = impl_item
        .trait_ref
        .as_ref()
        .and_then(|p| p.as_name())
        .unwrap_or_else(|| interner.intern(""));
    let trait_def_id = if trait_name != interner.intern("") {
        Some(TraitDefId::from_raw(0))
    } else {
        None
    };
    let self_ty = Ty::ERROR;
    let substs = Substitution::empty();
    ResolvedImplHeader {
        trait_def_id,
        trait_name: Some(trait_name),
        trait_substs: substs,
        self_ty,
        self_type_name: None,
        generic_param_names: vec![],
        polarity: ImplPolarity::Positive,
        span: Span::DUMMY,
    }
}

// ---------------------------------------------------------------------------
// Test helpers (copied from original, but adapted)
// ---------------------------------------------------------------------------
fn build_def_map(interner: &mut Interner, krate: CrateId, local_type_names: &[&str]) -> CrateDefMap {
    let mut scope = ItemScope::default();
    for &name_str in local_type_names {
        let name = interner.intern(name_str);
        scope.types.push((
            name,
            LocalDefId::from_raw(0),
            Visibility::Public,
            Span::DUMMY,
        ));
    }
    let root_id = ModuleId::from_raw(0);
    let root_data = ModuleData {
        parent: None,
        children: vec![],
        scope,
        origin: ModuleOrigin::CrateRoot,
        span: Span::DUMMY,
        def_id: LocalDefId::from_raw(0),
        visibility: Visibility::Public,
    };
    let mut modules = glyim_core::arena::IndexVec::new();
    modules.push(root_data);
    CrateDefMap {
        root: root_id,
        modules,
        krate,
        interner: interner.clone(),
    }
}

fn make_impl_item(interner: &mut Interner, trait_name: &str, self_ty_name: &str) -> ImplItem {
    let trait_path = Path::from_single(interner.intern(trait_name));
    let self_ty_path = Path::from_single(interner.intern(self_ty_name));
    ImplItem {
        trait_ref: Some(trait_path),
        self_ty: TypeRef::Path(self_ty_path),
        methods: vec![],
        generic_params: vec![],
        where_clauses: vec![],
    }
}

fn make_blanket_impl_item(interner: &mut Interner, trait_name: &str, param_name: &str) -> ImplItem {
    let trait_path = Path::from_single(interner.intern(trait_name));
    let param = interner.intern(param_name);
    let self_ty_path = Path::from_single(param);
    ImplItem {
        trait_ref: Some(trait_path),
        self_ty: TypeRef::Path(self_ty_path),
        methods: vec![],
        generic_params: vec![glyim_hir::GenericParam {
            name: param,
            kind: glyim_hir::GenericParamKind::Type { default: None },
            span: Span::DUMMY,
        }],
        where_clauses: vec![],
    }
}

// ---------------------------------------------------------------------------
// Tests (all now properly using ctx and the helper)
// ---------------------------------------------------------------------------

#[test]
fn t01_duplicate_impl_should_error() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &["MyType"]);
    let mut ctx = make_ty_ctx();
    let mut checker = CoherenceChecker::new(&def_map);

    let impl1 = make_impl_item(&mut interner, "Send", "MyType");
    let impl2 = make_impl_item(&mut interner, "Send", "MyType");

    let result1 = checker.check_and_register_impl_compat(
        &impl_item_to_header(&impl1, &mut interner, &mut ctx, &def_map),
        ImplPolarity::Positive,
        &ctx,
    );
    assert!(result1.is_ok(), "first impl should be accepted");

    let result2 = checker.check_and_register_impl_compat(
        &impl_item_to_header(&impl2, &mut interner, &mut ctx, &def_map),
        ImplPolarity::Positive,
        &ctx,
    );
    assert!(result2.is_err(), "duplicate impl should be rejected");
    let errors = result2.unwrap_err();
    assert!(!errors.is_empty());
    let msg = &errors[0].message;
    assert!(
        msg.contains("conflict") || msg.contains("overlap") || msg.contains("duplicate"),
        "expected conflict message, got: {}",
        msg
    );
}

#[test]
fn t02_orphan_rule_foreign_trait_foreign_type_error() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &[]);
    let mut ctx = make_ty_ctx();
    let checker = CoherenceChecker::new(&def_map);

    let impl_item = make_impl_item(&mut interner, "ForeignTrait", "ForeignType");
    let result = checker.check_orphan_rule(&impl_item_to_header(&impl_item, &mut interner, &mut ctx, &def_map));
    assert!(
        result.is_err(),
        "orphan rule should reject foreign trait + foreign type"
    );
    let errors = result.unwrap_err();
    assert!(errors[0].message.contains("orphan rule"));
}

#[test]
fn t03_blanket_impl_conflicts_with_concrete() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &["i32", "T"]);
    let mut ctx = make_ty_ctx();
    let mut checker = CoherenceChecker::new(&def_map);

    let concrete = make_impl_item(&mut interner, "MyTrait", "i32");
    let blanket = make_blanket_impl_item(&mut interner, "MyTrait", "T");

    checker
        .check_and_register_impl_compat(
            &impl_item_to_header(&concrete, &mut interner, &mut ctx, &def_map),
            ImplPolarity::Positive,
            &ctx,
        )
        .unwrap();

    let result = checker.check_and_register_impl_compat(
        &impl_item_to_header(&blanket, &mut interner, &mut ctx, &def_map),
        ImplPolarity::Positive,
        &ctx,
    );
    assert!(
        result.is_err(),
        "blanket impl should conflict with concrete"
    );
}

#[test]
fn t04_valid_orphan_foreign_trait_local_type() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &["LocalType"]);
    let mut ctx = make_ty_ctx();
    let checker = CoherenceChecker::new(&def_map);

    let impl_item = make_impl_item(&mut interner, "ForeignTrait", "LocalType");
    let result = checker.check_orphan_rule(&impl_item_to_header(&impl_item, &mut interner, &mut ctx, &def_map));
    assert!(
        result.is_ok(),
        "orphan rule should accept foreign trait + local type"
    );
}

#[test]
fn t05_negative_impl_overrides_auto_trait() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &["MyType"]);
    let mut ctx = make_ty_ctx();
    let mut checker = CoherenceChecker::new(&def_map);

    let neg_impl = make_impl_item(&mut interner, "Send", "MyType");
    let result = checker.check_and_register_impl_compat(
        &impl_item_to_header(&neg_impl, &mut interner, &mut ctx, &def_map),
        ImplPolarity::Negative,
        &ctx,
    );
    assert!(result.is_ok(), "negative impl should be allowed");
}

#[test]
fn t06_duplicate_with_different_polarity_error() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &["MyType"]);
    let mut ctx = make_ty_ctx();
    let mut checker = CoherenceChecker::new(&def_map);

    let pos_impl = make_impl_item(&mut interner, "Send", "MyType");
    let neg_impl = make_impl_item(&mut interner, "Send", "MyType");

    checker
        .check_and_register_impl_compat(
            &impl_item_to_header(&pos_impl, &mut interner, &mut ctx, &def_map),
            ImplPolarity::Positive,
            &ctx,
        )
        .unwrap();

    let result = checker.check_and_register_impl_compat(
        &impl_item_to_header(&neg_impl, &mut interner, &mut ctx, &def_map),
        ImplPolarity::Negative,
        &ctx,
    );
    assert!(
        result.is_err(),
        "impl with opposite polarity should conflict"
    );
}

#[test]
fn t07_orphan_local_trait_foreign_type_allowed() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &["MyTrait"]);
    let mut ctx = make_ty_ctx();
    let checker = CoherenceChecker::new(&def_map);

    let impl_item = make_impl_item(&mut interner, "MyTrait", "ForeignType");
    let result = checker.check_orphan_rule(&impl_item_to_header(&impl_item, &mut interner, &mut ctx, &def_map));
    assert!(
        result.is_ok(),
        "orphan rule should allow local trait on foreign type"
    );
}

#[test]
fn t08_two_non_overlapping_blanket_impls_allowed() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &["A", "B"]);
    let mut ctx = make_ty_ctx();
    let mut checker = CoherenceChecker::new(&def_map);

    let blanket_a = make_blanket_impl_item(&mut interner, "From", "A");
    let blanket_b = make_blanket_impl_item(&mut interner, "From", "B");

    let r1 = checker.check_and_register_impl_compat(
        &impl_item_to_header(&blanket_a, &mut interner, &mut ctx, &def_map),
        ImplPolarity::Positive,
        &ctx,
    );
    assert!(r1.is_ok(), "first blanket impl should be accepted");

    let r2 = checker.check_and_register_impl_compat(
        &impl_item_to_header(&blanket_b, &mut interner, &mut ctx, &def_map),
        ImplPolarity::Positive,
        &ctx,
    );
    assert!(
        r2.is_ok(),
        "second blanket impl with different param should be accepted"
    );
}

#[test]
fn t09_negative_impl_orphan_error() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &[]);
    let mut ctx = make_ty_ctx();
    let checker = CoherenceChecker::new(&def_map);

    let neg_impl = make_impl_item(&mut interner, "ForeignTrait", "ForeignType");
    let result = checker.check_orphan_rule(&impl_item_to_header(&neg_impl, &mut interner, &mut ctx, &def_map));
    assert!(
        result.is_err(),
        "negative impl for foreign trait + foreign type should violate orphan rule"
    );
}

#[test]
fn t10_different_traits_no_conflict() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &["MyType"]);
    let mut ctx = make_ty_ctx();
    let mut checker = CoherenceChecker::new(&def_map);

    let impl_trait_a = make_impl_item(&mut interner, "TraitA", "MyType");
    let impl_trait_b = make_impl_item(&mut interner, "TraitB", "MyType");

    let r1 = checker.check_and_register_impl_compat(
        &impl_item_to_header(&impl_trait_a, &mut interner, &mut ctx, &def_map),
        ImplPolarity::Positive,
        &ctx,
    );
    let r2 = checker.check_and_register_impl_compat(
        &impl_item_to_header(&impl_trait_b, &mut interner, &mut ctx, &def_map),
        ImplPolarity::Positive,
        &ctx,
    );

    assert!(r1.is_ok(), "impl for TraitA should be accepted");
    assert!(r2.is_ok(), "impl for TraitB should be accepted");
}
