//! Tests for cast handling and Coerce predicate.

use glyim_test::{AnalysisTester, assert_no_errors};

#[test]
fn test_cast_integer_to_float() {
    let source = r#"
        fn main() {
            let x = 42 as f64;
            let y = x + 0.5;
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert_no_errors(&result.diagnostics);
}

#[test]
fn test_cast_float_to_integer() {
    let source = r#"
        fn main() {
            let x = 3.14 as i32;
            let y = x + 1;
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert_no_errors(&result.diagnostics);
}

#[test]
fn test_cast_pointer_to_int() {
    let source = r#"
        fn main() {
            let ptr: *const u8 = 0 as *const u8;
            let addr = ptr as usize;
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert_no_errors(&result.diagnostics);
}

#[test]
fn test_coerce_mut_to_immut() {
    let source = r#"
        fn takes_immut(x: &i32) -> i32 { *x }
        fn main() {
            let mut a = 5;
            let b = &mut a;
            let c = takes_immut(b); // Should coerce &mut i32 to &i32
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    // This requires the Coerce predicate to be handled.
    // Currently our type checker does not implement coercion.
    // We will add handling for Coerce predicate in the unification step.
    // The test will initially fail, then we implement.
    assert_no_errors(&result.diagnostics);
}

#[test]
fn test_coerce_array_to_slice() {
    let source = r#"
        fn sum_slice(s: &[i32]) -> i32 {
            let mut acc = 0;
            for &x in s { acc += x; }
            acc
        }
        fn main() {
            let arr = [1, 2, 3];
            let total = sum_slice(&arr);
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert_no_errors(&result.diagnostics);
}
