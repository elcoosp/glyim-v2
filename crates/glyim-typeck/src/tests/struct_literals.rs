//! Tests for struct literal type checking (S17-T01, S17-T02)
use glyim_test::{test_ty_ctx, with_fresh_ty_ctx, assert_ty, assert_no_errors};
use glyim_test::phase::AnalysisTester;
use glyim_core::primitives::{IntTy, Mutability};
use glyim_type::{Ty, TyKind};

#[test]
fn s17_t01_struct_literal_basic() {
    // Point { x: 1, y: 2 } type checks and resolves fields
    let mut tester = AnalysisTester::new();
    let source = r#"
struct Point { x: i32, y: i32 }
fn main() {
    let p = Point { x: 1, y: 2 };
}
"#;
    let result = tester.run_source(source);
    assert_no_errors(&result.diagnostics);
}

#[test]
fn s17_t01_struct_literal_field_order() {
    // Fields can be specified in any order
    let mut tester = AnalysisTester::new();
    let source = r#"
struct Point { x: i32, y: i32 }
fn main() {
    let p = Point { y: 2, x: 1 };
}
"#;
    let result = tester.run_source(source);
    assert_no_errors(&result.diagnostics);
}

#[test]
fn s17_t02_struct_update_syntax() {
    // Point { x: 1, ..base } copies remaining fields from base
    let mut tester = AnalysisTester::new();
    let source = r#"
struct Point { x: i32, y: i32 }
fn main() {
    let base = Point { x: 0, y: 0 };
    let p = Point { x: 1, ..base };
}
"#;
    let result = tester.run_source(source);
    assert_no_errors(&result.diagnostics);
}

#[test]
fn s17_t02_struct_update_missing_field_error() {
    // Error when required field is missing without spread
    let mut tester = AnalysisTester::new();
    let source = r#"
struct Point { x: i32, y: i32 }
fn main() {
    let p = Point { x: 1 };
}
"#;
    let result = tester.run_source(source);
    assert!(result.diagnostics.iter().any(|d| d.message.contains("missing field")));
}

#[test]
fn s17_t01_struct_literal_type_mismatch() {
    // Error when field type doesn't match
    let mut tester = AnalysisTester::new();
    let source = r#"
struct Point { x: i32, y: i32 }
fn main() {
    let p = Point { x: 1, y: "hello" };
}
"#;
    let result = tester.run_source(source);
    assert!(result.diagnostics.iter().any(|d| d.message.contains("mismatch")));
}

#[test]
fn s17_t01_struct_literal_unknown_field() {
    // Error when unknown field is specified
    let mut tester = AnalysisTester::new();
    let source = r#"
struct Point { x: i32, y: i32 }
fn main() {
    let p = Point { x: 1, y: 2, z: 3 };
}
"#;
    let result = tester.run_source(source);
    assert!(result.diagnostics.iter().any(|d| d.message.contains("no field")));
}
