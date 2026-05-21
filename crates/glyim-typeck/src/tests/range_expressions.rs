//! Tests for range expression type checking (S17-T04)
use glyim_test::phase::AnalysisTester;
use glyim_test::assert_no_errors;

#[test]
fn s17_t04_range_inclusive_i32() {
    // 1..=5 type checks as RangeInclusive<i32>
    let mut tester = AnalysisTester::new();
    let source = r#"
fn main() {
    let r = 1..=5;
    for i in r {
        let _x: i32 = i;
    }
}
"#;
    let result = tester.run_source(source);
    assert_no_errors(&result.diagnostics);
}

#[test]
fn s17_t04_range_exclusive_i32() {
    // 1..5 type checks as Range<i32>
    let mut tester = AnalysisTester::new();
    let source = r#"
fn main() {
    let r = 1..5;
    for i in r {
        let _x: i32 = i;
    }
}
"#;
    let result = tester.run_source(source);
    assert_no_errors(&result.diagnostics);
}

#[test]
fn s17_t04_range_with_vars() {
    // Ranges work with variable bounds
    let mut tester = AnalysisTester::new();
    let source = r#"
fn main() {
    let start: i32 = 0;
    let end: i32 = 10;
    let r = start..end;
    for i in r {
        let _x: i32 = i;
    }
}
"#;
    let result = tester.run_source(source);
    assert_no_errors(&result.diagnostics);
}

#[test]
fn s17_t04_range_type_mismatch() {
    // Error when range bounds have different types
    let mut tester = AnalysisTester::new();
    let source = r#"
fn main() {
    let r = 1..=5u64;
}
"#;
    let result = tester.run_source(source);
    // Should have type mismatch error
    assert!(result.diagnostics.iter().any(|d| d.message.contains("mismatch")));
}

#[test]
fn s17_t04_range_unbounded_start() {
    // ..5 is valid (unbounded start)
    let mut tester = AnalysisTester::new();
    let source = r#"
fn main() {
    let r = ..5;
}
"#;
    let result = tester.run_source(source);
    assert_no_errors(&result.diagnostics);
}

#[test]
fn s17_t04_range_unbounded_end() {
    // 5.. is valid (unbounded end)
    let mut tester = AnalysisTester::new();
    let source = r#"
fn main() {
    let r = 5..;
}
"#;
    let result = tester.run_source(source);
    assert_no_errors(&result.diagnostics);
}
