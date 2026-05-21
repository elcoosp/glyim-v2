//! Tests for match guard lowering (S16-T03)
//!
//! NOTE: These tests are currently ignored because the Glyim frontend/typeck
//! does not yet properly support match guards with pattern bindings.
//! Once typeck supports guard expressions with bound variables, enable these.

#[test]
#[ignore = "requires frontend/typeck support for guard bindings (S03 dependency)"]
fn lower_guard_branch() {
    // When enabled, this test should verify:
    // match opt { Some(x) if x > 0 => x, _ => 0 }
    // lowers to: switch on discriminant -> if Some, evaluate guard -> branch.
    panic!("Test ignored - enable when typeck supports guard bindings");
}
