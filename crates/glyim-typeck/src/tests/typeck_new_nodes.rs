use glyim_test::phase::AnalysisTester;
use glyim_test::assert_no_errors;
use glyim_test::assert_has_errors;
use glyim_test::assert_diag_contains;

#[test]
fn match_guard_uses_binding() {
    let trace = AnalysisTester::new(
        r#"
        fn main() {
            let x = Some(5);
            match x {
                Some(y) if y > 0 => {},
                _ => {}
            }
        }
        "#,
    )
    .run();
    assert_no_errors(&trace.typeck_diagnostics);
}

#[test]
fn or_pattern_same_types() {
    let trace = AnalysisTester::new(
        r#"
        fn main() {
            match 1 {
                1 | 2 => {},
                _ => {}
            }
        }
        "#,
    )
    .run();
    assert_no_errors(&trace.typeck_diagnostics);
}

#[test]
fn range_pattern_integer() {
    let trace = AnalysisTester::new(
        r#"
        fn main() {
            match 5 {
                1..=10 => {},
                _ => {}
            }
        }
        "#,
    )
    .run();
    assert_no_errors(&trace.typeck_diagnostics);
}

#[test]
fn slice_pattern_array() {
    let trace = AnalysisTester::new(
        r#"
        fn main() {
            let arr = [1, 2, 3];
            match arr {
                [a, b, ..rest] => {},
                _ => {}
            }
        }
        "#,
    )
    .run();
    assert_no_errors(&trace.typeck_diagnostics);
}

#[test]
fn index_expression_array() {
    let trace = AnalysisTester::new(
        r#"
        fn main() {
            let arr = [1, 2, 3];
            let x = arr[0];
        }
        "#,
    )
    .run();
    assert_no_errors(&trace.typeck_diagnostics);
}

#[test]
fn struct_literal_with_spread() {
    let trace = AnalysisTester::new(
        r#"
        struct S { x: i32, y: i32 }
        fn main() {
            let a = S { x: 1, y: 2 };
            let b = S { x: 3, ..a };
        }
        "#,
    )
    .run();
    assert_no_errors(&trace.typeck_diagnostics);
}

#[test]
fn or_pattern_mismatched_types_fails() {
    let trace = AnalysisTester::new(
        r#"
        fn main() {
            match 1 {
                true | "hello" => {},
                _ => {}
            }
        }
        "#,
    )
    .run();
    assert_has_errors(&trace.typeck_diagnostics);
    assert_diag_contains(&trace.typeck_diagnostics, "mismatched types");
}
