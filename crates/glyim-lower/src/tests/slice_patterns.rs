//! Tests for slice pattern lowering (S16-T04)
//!
//! NOTE: These tests are currently ignored because the Glyim parser
//! does not yet support slice patterns (`[a, b]`) in match expressions.
//! Once the parser and typeck support Pat::Slice, enable these tests.

#[test]
#[ignore = "requires parser/typeck support for Pat::Slice (S03 dependency)"]
fn lower_slice_pattern_binding() {
    // When enabled, this test should verify:
    // match slice { [a, b] => *a + *b, _ => 0 }
    // lowers to: Len check, Index projections, bindings for a and b.
    panic!("Test ignored - enable when parser supports slice patterns");
}
