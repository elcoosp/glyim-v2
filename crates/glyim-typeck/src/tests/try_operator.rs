//! Tests for ? operator desugaring (S17-T03)
use glyim_test::phase::AnalysisTester;
use glyim_test::assert_no_errors;

#[test]
fn s17_t03_try_operator_result_ok() {
    // parse()?.value desugars to match on Result - success path
    let mut tester = AnalysisTester::new();
    let source = r#"
struct Config { value: i32 }
fn parse() -> Result<Config, String> {
    Ok(Config { value: 42 })
}
fn main() -> Result<(), String> {
    let v = parse()?.value;
    Ok(())
}
"#;
    let result = tester.run_source(source);
    assert_no_errors(&result.diagnostics);
}

#[test]
fn s17_t03_try_operator_result_err() {
    // ? operator propagates error - type checking still works
    let mut tester = AnalysisTester::new();
    let source = r#"
fn parse() -> Result<i32, String> {
    Err("error".to_string())
}
fn main() -> Result<(), String> {
    let v: i32 = parse()?;
    Ok(())
}
"#;
    let result = tester.run_source(source);
    assert_no_errors(&result.diagnostics);
}

#[test]
fn s17_t03_try_operator_wrong_return_type() {
    // Error when ? used in function that doesn't return Result
    let mut tester = AnalysisTester::new();
    let source = r#"
fn parse() -> Result<i32, String> {
    Ok(42)
}
fn main() {
    let v: i32 = parse()?;
}
"#;
    let result = tester.run_source(source);
    // Should error: ? requires function to return Result/Option
    assert!(result.diagnostics.iter().any(|d|
        d.message.contains("try") || d.message.contains("Result") || d.message.contains("return type")
    ));
}

#[test]
fn s17_t03_try_operator_option() {
    // ? also works with Option
    let mut tester = AnalysisTester::new();
    let source = r#"
fn get_value() -> Option<i32> {
    Some(42)
}
fn main() -> Option<()> {
    let v: i32 = get_value()?;
    Some(())
}
"#;
    let result = tester.run_source(source);
    assert_no_errors(&result.diagnostics);
}
