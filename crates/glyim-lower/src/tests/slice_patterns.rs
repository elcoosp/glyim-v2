//! Slice pattern lowering tests.
//!
//! The lowering implementation for `Pat::Slice` in match arms is complete.
//! However, the frontend (parsing and type checking) does not yet support slice pattern syntax.
//! Once the frontend is ready (streams W1-C03 and W2-C01), these tests can be enabled.
//! The tests are ignored for now but serve as documentation and will be re-enabled later.

use glyim_test::phase::MirGenTester;
use glyim_test::assert_no_errors;

#[test]
#[ignore = "frontend slice pattern syntax not yet implemented (requires W1-C03 and W2-C01)"]
fn slice_pattern_fixed_length() {
    let src = r#"
    fn main() {
        let arr = [1, 2, 3];
        match &arr[..] {
            [a, b, c] => { let _ = a + b + c; }
            _ => {}
        }
    }
    "#;
    let trace = MirGenTester::new(src).run().unwrap();
    assert_no_errors(&trace.diagnostics);
}

#[test]
#[ignore = "frontend slice pattern syntax not yet implemented (requires W1-C03 and W2-C01)"]
fn slice_pattern_empty() {
    let src = r#"
    fn main() {
        let arr: [i32; 0] = [];
        match &arr[..] {
            [] => {}
            _ => {}
        }
    }
    "#;
    let trace = MirGenTester::new(src).run().unwrap();
    assert_no_errors(&trace.diagnostics);
}

#[test]
#[ignore = "frontend slice pattern syntax not yet implemented (requires W1-C03 and W2-C01)"]
fn slice_pattern_rest_only() {
    let src = r#"
    fn main() {
        let arr = [1, 2, 3];
        match &arr[..] {
            [..] => {}
            _ => {}
        }
    }
    "#;
    let trace = MirGenTester::new(src).run().unwrap();
    assert_no_errors(&trace.diagnostics);
}
