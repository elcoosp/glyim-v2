//! Tests for coherence and orphan rules (Stream V04).

use glyim_core::def_id::{CrateId, LocalDefId};
use glyim_core::interner::{Interner, Name};
use glyim_core::primitives::Visibility;
use glyim_def_map::{CrateDefMap, ModuleData, ModuleId, ModuleOrigin, ItemScope};
use glyim_hir::{ImplItem, TypeRef};
use glyim_span::Span;
use glyim_type::ImplPolarity;

use crate::coherence::CoherenceChecker;

/// Build a CrateDefMap using the given interner and seed the root scope with local type names.
fn build_def_map(interner: &mut Interner, krate: CrateId, local_type_names: &[&str]) -> CrateDefMap {
    let mut scope = ItemScope {
        types: vec![],
        values: vec![],
        macros: vec![],
    };
    for &name_str in local_type_names {
        let name = interner.intern(name_str);
        scope.types.push((name, LocalDefId::from_raw(0), Visibility::Public, Span::DUMMY));
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

/// Create a simple ImplItem using the given interner.
fn make_impl_item(interner: &mut Interner, trait_name: &str, self_ty_name: &str) -> ImplItem {
    let trait_path = glyim_hir::Path::from_single(interner.intern(trait_name));
    let self_ty_path = glyim_hir::Path::from_single(interner.intern(self_ty_name));
    ImplItem {
        trait_ref: Some(trait_path),
        self_ty: TypeRef::Path(self_ty_path),
        methods: vec![],
        generic_params: vec![],
        where_clauses: vec![],
    }
}

/// Create a blanket ImplItem (with a single generic type parameter) using the given interner.
fn make_blanket_impl_item(interner: &mut Interner, trait_name: &str, param_name: &str) -> ImplItem {
    let trait_path = glyim_hir::Path::from_single(interner.intern(trait_name));
    let param_name = interner.intern(param_name);
    let self_ty_path = glyim_hir::Path::from_single(param_name);
    ImplItem {
        trait_ref: Some(trait_path),
        self_ty: TypeRef::Path(self_ty_path),
        methods: vec![],
        generic_params: vec![
            glyim_hir::GenericParam {
                name: param_name,
                kind: glyim_hir::GenericParamKind::Type { default: None },
                span: Span::DUMMY,
            }
        ],
        where_clauses: vec![],
    }
}

// ============================================================================
// V04-T01: Duplicate impl for same type -> error
// ============================================================================
#[test]
fn t01_duplicate_impl_should_error() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &["MyType"]);
    let mut checker = CoherenceChecker::new(&def_map, local_krate);

    let impl1 = make_impl_item(&mut interner, "Send", "MyType");
    let impl2 = make_impl_item(&mut interner, "Send", "MyType");

    let result1 = checker.check_and_register_impl(&impl1, ImplPolarity::Positive);
    assert!(result1.is_ok(), "first impl should be accepted");

    let result2 = checker.check_and_register_impl(&impl2, ImplPolarity::Positive);
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

// ============================================================================
// V04-T02: Orphan rule: foreign trait on foreign type -> error
// ============================================================================
#[test]
fn t02_orphan_rule_foreign_trait_foreign_type_error() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    // Do not seed "ForeignType" → it's foreign
    let def_map = build_def_map(&mut interner, local_krate, &[]);
    let checker = CoherenceChecker::new(&def_map, local_krate);

    let impl_item = make_impl_item(&mut interner, "ForeignTrait", "ForeignType");
    let result = checker.check_orphan_rule(&impl_item, ImplPolarity::Positive);
    assert!(result.is_err(), "orphan rule should reject foreign trait + foreign type");
    let errors = result.unwrap_err();
    assert!(errors[0].message.contains("orphan rule"));
}

// ============================================================================
// V04-T03: Blanket impl conflicts with concrete impl -> error
// ============================================================================
#[test]
fn t03_blanket_impl_conflicts_with_concrete() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    // Seed both "i32" and "T" as local so orphan rule passes for both concrete and blanket
    let def_map = build_def_map(&mut interner, local_krate, &["i32", "T"]);
    let mut checker = CoherenceChecker::new(&def_map, local_krate);

    let concrete = make_impl_item(&mut interner, "MyTrait", "i32");
    let blanket = make_blanket_impl_item(&mut interner, "MyTrait", "T");

    checker.check_and_register_impl(&concrete, ImplPolarity::Positive).unwrap();

    let result = checker.check_and_register_impl(&blanket, ImplPolarity::Positive);
    assert!(result.is_err(), "blanket impl should conflict with concrete");
}

// ============================================================================
// V04-T04: Valid orphan: impl ForeignTrait for LocalType -> ok
// ============================================================================
#[test]
fn t04_valid_orphan_foreign_trait_local_type() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &["LocalType"]);
    let checker = CoherenceChecker::new(&def_map, local_krate);

    let impl_item = make_impl_item(&mut interner, "ForeignTrait", "LocalType");
    let result = checker.check_orphan_rule(&impl_item, ImplPolarity::Positive);
    assert!(result.is_ok(), "orphan rule should accept foreign trait + local type");
}

// ============================================================================
// V04-T05: Negative impl `impl !Send for MyType` -> overrides auto trait
// ============================================================================
#[test]
fn t05_negative_impl_overrides_auto_trait() {
    let local_krate = CrateId::from_raw(0);
    let mut interner = Interner::new();
    let def_map = build_def_map(&mut interner, local_krate, &["MyType"]);
    let mut checker = CoherenceChecker::new(&def_map, local_krate);

    let neg_impl = make_impl_item(&mut interner, "Send", "MyType");
    let result = checker.check_and_register_impl(&neg_impl, ImplPolarity::Negative);
    assert!(result.is_ok(), "negative impl should be allowed");

    assert!(
        checker.has_negative_impl("Send", "MyType"),
        "should have recorded negative impl"
    );
}
