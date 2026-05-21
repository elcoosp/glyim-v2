//! Tests for or-pattern lowering (S16-T01)
//!
//! NOTE: These tests are currently ignored because the Glyim frontend/typeck
//! does not yet support or-patterns (`0 | 1`) in match expressions.
//! Once typeck supports Pat::Or, these tests can be enabled to verify
//! that lowering generates SwitchInt terminators correctly.

#[test]
#[ignore = "requires frontend/typeck support for Pat::Or (S03 dependency)"]
fn lower_or_pattern_to_switch() {
    // When enabled, this test should verify:
    // match x { 0 | 1 => true, _ => false }
    // lowers to a SwitchInt with targets for 0 and 1 jumping to the same arm block.
    panic!("Test ignored - enable when typeck supports or-patterns");
}
