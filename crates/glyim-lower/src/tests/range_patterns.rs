//! Tests for range pattern lowering (S16-T02)
//!
//! NOTE: These tests are currently ignored because the Glyim frontend/typeck
//! does not yet support range patterns (`0..=9`) in match expressions.
//! Once typeck supports Pat::Range, these tests can be enabled.

#[test]
#[ignore = "requires frontend/typeck support for Pat::Range (S03 dependency)"]
fn lower_range_pattern() {
    // When enabled, this test should verify:
    // match x { 0..=9 => true, _ => false }
    // lowers to SwitchInt with values 0-9 or a comparison chain.
    panic!("Test ignored - enable when typeck supports range patterns");
}
