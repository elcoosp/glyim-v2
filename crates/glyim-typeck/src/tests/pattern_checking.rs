//! Tests for pattern checking: Or patterns, Range patterns, Struct patterns.

use glyim_test::{AnalysisTester, assert_no_errors};

#[test]
fn test_or_pattern() {
    let source = r#"
        enum Option<T> { Some(T), None }
        fn main() {
            let x = Option::Some(42);
            match x {
                Option::Some(0) | Option::None => { },
                Option::Some(n) => { let _ = n; },
            }
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert_no_errors(&result.diagnostics);
}

#[test]
fn test_or_pattern_different_types() {
    let source = r#"
        enum Result<T, E> { Ok(T), Err(E) }
        fn main() {
            let res = Result::Ok::<i32, &str>(10);
            match res {
                Result::Ok(x) | Result::Err(_) => { let _ = x; },
            }
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    // The or pattern binds `x` in both arms – types must be compatible.
    // For `Result`, the `Ok` variant has type `T`, `Err` has `E`, so not compatible.
    // We expect an error.
    assert!(!result.diagnostics.is_empty());
    let has_type_mismatch = result.diagnostics.iter().any(|d| d.message.contains("type mismatch") || d.message.contains("incompatible patterns"));
    assert!(has_type_mismatch);
}

#[test]
fn test_range_pattern() {
    let source = r#"
        fn main() {
            let x = 5;
            match x {
                0..=5 => { },
                6..=10 => { },
                _ => { },
            }
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert_no_errors(&result.diagnostics);
}

#[test]
fn test_range_pattern_char() {
    let source = r#"
        fn main() {
            let c = 'a';
            match c {
                'a'..='z' => { },
                'A'..='Z' => { },
                _ => { },
            }
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert_no_errors(&result.diagnostics);
}

#[test]
fn test_struct_pattern() {
    let source = r#"
        struct Point { x: i32, y: i32 }
        fn main() {
            let p = Point { x: 10, y: 20 };
            match p {
                Point { x: 0, y } => { let _ = y; },
                Point { x, y: 0 } => { let _ = x; },
                Point { x, y } => { let _ = x + y; },
            }
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert_no_errors(&result.diagnostics);
}

#[test]
fn test_struct_pattern_with_rest() {
    let source = r#"
        struct Point3D { x: i32, y: i32, z: i32 }
        fn main() {
            let p = Point3D { x: 1, y: 2, z: 3 };
            match p {
                Point3D { x, .. } => { let _ = x; },
            }
        }
    "#;
    let mut tester = AnalysisTester::new(source);
    let result = tester.typeck();
    assert_no_errors(&result.diagnostics);
}
