//! S12-T04: Object safety checks reject self-types correctly.

use crate::object_safety::*;
use glyim_core::interner::Interner;
use glyim_span::{Span, SyntaxContext, ByteIdx, FileId};

fn test_span() -> Span {
    Span::new(FileId::BOGUS, ByteIdx::ZERO, ByteIdx::ZERO, SyntaxContext::ROOT)
}

fn named(s: &str) -> glyim_core::interner::Name {
    Interner::default().intern(s)
}

// ---- Self: Sized trait is not object-safe ----

#[test]
fn self_sized_trait_is_not_object_safe() {
    let violations = check_object_safety(true, &[]);
    assert_eq!(violations.len(), 1);
    assert!(matches!(violations[0], ObjectSafetyViolation::SelfSized));
}

// ---- Trait with only &self methods is object-safe ----

#[test]
fn trait_with_ref_self_only_is_object_safe() {
    let methods = vec![MethodSignature {
        name: named("method"),
        span: test_span(),
        self_kind: MethodSelfKind::ByReference,
        has_generic_params: false,
        returns_self: false,
    }];
    let violations = check_object_safety(false, &methods);
    assert!(violations.is_empty(), "trait with only &self methods should be object-safe");
}

// ---- Trait with generic method is not object-safe ----

#[test]
fn generic_method_is_not_object_safe() {
    let methods = vec![MethodSignature {
        name: named("generic_fn"),
        span: test_span(),
        self_kind: MethodSelfKind::ByReference,
        has_generic_params: true,
        returns_self: false,
    }];
    let violations = check_object_safety(false, &methods);
    assert!(violations.iter().any(|v| matches!(v, ObjectSafetyViolation::GenericMethod { .. })));
}

// ---- Trait with static method (no self) is not object-safe ----

#[test]
fn static_method_is_not_object_safe() {
    let methods = vec![MethodSignature {
        name: named("new"),
        span: test_span(),
        self_kind: MethodSelfKind::None,
        has_generic_params: false,
        returns_self: false,
    }];
    let violations = check_object_safety(false, &methods);
    assert!(violations.iter().any(|v| matches!(v, ObjectSafetyViolation::StaticMethod { .. })));
}

// ---- Trait with self by value (without Self: Sized) is not object-safe ----

#[test]
fn by_value_self_without_sized_is_not_object_safe() {
    let methods = vec![MethodSignature {
        name: named("into_inner"),
        span: test_span(),
        self_kind: MethodSelfKind::ByValue,
        has_generic_params: false,
        returns_self: false,
    }];
    let violations = check_object_safety(false, &methods);
    assert!(violations.iter().any(|v| matches!(v, ObjectSafetyViolation::ByValueSelf { .. })));
}

// ---- Self: Sized with by-value self is only SelfSized violation ----

#[test]
fn self_sized_with_by_value_self_only_flags_self_sized() {
    let methods = vec![MethodSignature {
        name: named("into_inner"),
        span: test_span(),
        self_kind: MethodSelfKind::ByValue,
        has_generic_params: false,
        returns_self: false,
    }];
    let violations = check_object_safety(true, &methods);
    // Should only have SelfSized, not ByValueSelf (since Self: Sized already covers it)
    assert!(violations.iter().any(|v| matches!(v, ObjectSafetyViolation::SelfSized)));
    assert!(!violations.iter().any(|v| matches!(v, ObjectSafetyViolation::ByValueSelf { .. })));
}

// ---- Multiple violations ----

#[test]
fn multiple_violations_reported() {
    let methods = vec![
        MethodSignature {
            name: named("generic_fn"),
            span: test_span(),
            self_kind: MethodSelfKind::ByReference,
            has_generic_params: true,
            returns_self: false,
        },
        MethodSignature {
            name: named("static_fn"),
            span: test_span(),
            self_kind: MethodSelfKind::None,
            has_generic_params: false,
            returns_self: false,
        },
    ];
    let violations = check_object_safety(false, &methods);
    assert!(violations.len() >= 2, "should have at least 2 violations");
}

// ---- Empty trait is object-safe ----

#[test]
fn empty_trait_is_object_safe() {
    let violations = check_object_safety(false, &[]);
    assert!(violations.is_empty());
}

// ---- Mix of safe and unsafe methods ----

#[test]
fn mixed_methods_reports_only_violations() {
    let methods = vec![
        MethodSignature {
            name: named("safe_method"),
            span: test_span(),
            self_kind: MethodSelfKind::ByReference,
            has_generic_params: false,
            returns_self: false,
        },
        MethodSignature {
            name: named("unsafe_generic"),
            span: test_span(),
            self_kind: MethodSelfKind::ByReference,
            has_generic_params: true,
            returns_self: false,
        },
        MethodSignature {
            name: named("another_safe"),
            span: test_span(),
            self_kind: MethodSelfKind::ByReference,
            has_generic_params: false,
            returns_self: false,
        },
    ];
    let violations = check_object_safety(false, &methods);
    assert_eq!(violations.len(), 1, "only the generic method should be a violation");
    assert!(matches!(&violations[0], ObjectSafetyViolation::GenericMethod { method, .. } if *method == named("unsafe_generic")));
}
