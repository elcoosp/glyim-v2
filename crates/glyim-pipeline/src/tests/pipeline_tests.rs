use glyim_diag::CompResult;
use glyim_test::mock::{MockCodegen, TestDbBuilder};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::NamedTempFile;

use crate::Pipeline;

fn setup_db_with_tempfile(source: &str) -> (TestDbBuilder, NamedTempFile) {
    let mut tmp = NamedTempFile::new().expect("Failed to create temp file");
    write!(tmp, "{}", source).expect("Failed to write temp file");
    let path = tmp.path().to_path_buf();
    let mut builder = TestDbBuilder::new()
        .name("test_pipeline")
        .target_triple("x86_64-unknown-linux-gnu")
        .opt_level(0);
    builder = builder.file(path.clone(), Arc::from(source));
    (builder, tmp)
}

fn compile_with_mock(source: &str) -> (MockCodegen, CompResult<()>) {
    let (builder, tmp) = setup_db_with_tempfile(source);
    let backend = MockCodegen::new();
    let mut db = builder.build();
    let path = tmp.path().to_path_buf();
    let output_path = std::path::Path::new("test_output.o");
    let result = Pipeline::compile_file(&mut db, &path, &backend, output_path);
    (backend, result)
}

// === Original 6 tests ===

#[test]
fn compile_empty_file_ok() {
    let (_, result) = compile_with_mock("");
    assert!(result.is_ok(), "expected Ok, got {:?}", result.err());
}

#[test]
fn compile_simple_main_ok() {
    let (_, result) = compile_with_mock("fn main() {}");
    assert!(result.is_ok(), "expected Ok, got {:?}", result.err());
}

#[test]
fn compile_type_error_produces_diagnostics() {
    let (_, result) = compile_with_mock("fn main() { true + 1 }");
    match result {
        Err(diags) => {
            assert!(!diags.is_empty(), "expected diagnostics on type error");
            assert!(
                diags.iter().any(|d| d.is_error()),
                "should contain at least one error diagnostic"
            );
        }
        Ok(_) => panic!("expected compilation to fail with errors"),
    }
}

#[test]
fn compile_syntax_error_produces_diagnostics() {
    let (_, result) = compile_with_mock("fn main() {");
    match result {
        Err(diags) => {
            assert!(!diags.is_empty());
            assert!(diags.iter().any(|d| d.is_error()));
        }
        Ok(_) => panic!("expected compilation to fail"),
    }
}

#[test]
fn compile_missing_file_returns_io_error() {
    let backend = MockCodegen::new();
    let mut db = TestDbBuilder::new().name("test_missing").build();
    let path = PathBuf::from("nonexistent_xyz123.g");
    let output_path = std::path::Path::new("test_output.o");
    let result = Pipeline::compile_file(&mut db, &path, &backend, output_path);
    match result {
        Err(diags) => {
            assert!(!diags.is_empty());
            assert!(
                diags
                    .iter()
                    .any(|d| d.message.contains("I/O") || d.message.to_lowercase().contains("io"))
            );
        }
        Ok(_) => panic!("expected failure"),
    }
}

#[test]
fn backend_generate_is_called() {
    let (backend, result) = compile_with_mock("fn main() {}");
    assert!(result.is_ok());
    assert!(
        !backend.calls().is_empty(),
        "backend.generate was not invoked"
    );
}

// === New comprehensive tests ===

#[test]
fn multiple_functions_compile() {
    let (_, result) = compile_with_mock(
        "
fn foo() {}
fn bar() {}
fn main() {}
",
    );
    assert!(
        result.is_ok(),
        "multiple functions failed: {:?}",
        result.err()
    );
}

#[test]
fn binary_i32_add() {
    let (_, result) = compile_with_mock("fn main() { 1 + 2 }");
    assert!(result.is_ok(), "binary i32 add failed: {:?}", result.err());
}

#[test]
fn if_else_expression() {
    let (_, result) = compile_with_mock("fn main() { if true { 1 } else { 2 } }");
    assert!(result.is_ok(), "if/else failed: {:?}", result.err());
}

#[test]
fn block_expression() {
    let (_, result) = compile_with_mock("fn main() { { 42 } }");
    assert!(
        result.is_ok(),
        "block expression failed: {:?}",
        result.err()
    );
}

#[test]
fn reference_expression() {
    let (_, result) = compile_with_mock("fn main() { &42 }");
    assert!(
        result.is_ok(),
        "reference expression failed: {:?}",
        result.err()
    );
}

#[test]
fn multiple_params_and_return() {
    // Function with params and explicit return type that uses params in a binary expression
    let (_, result) = compile_with_mock("fn add(x: i32, y: i32) -> i32 { x + y } fn main() {}");
    assert!(result.is_ok(), "multiple params failed: {:?}", result.err());
}

#[test]
fn call_expression_unsupported_yet() {
    // Call expressions are not yet supported in typeck (hits wildcard arm).
    // This test documents that calling another function produces an error.
    let (_, result) =
        compile_with_mock("fn add(x: i32, y: i32) -> i32 { x + y } fn main() { add(1, 2) }");
    assert!(result.is_err(), "call expression should not compile yet");
}

#[test]
fn boolean_comparison() {
    let (_, result) = compile_with_mock("fn main() { true == false }");
    assert!(
        result.is_ok(),
        "boolean comparison failed: {:?}",
        result.err()
    );
}

#[test]
fn nested_blocks() {
    let (_, result) = compile_with_mock("fn main() { { { { 1 } } } }");
    assert!(result.is_ok(), "nested blocks failed: {:?}", result.err());
}

#[test]
fn if_else_mismatched_types_error() {
    let (_, result) = compile_with_mock("fn main() { if true { 1 } else { false } }");
    assert!(
        result.is_err(),
        "if/else mismatched types should be an error but succeeded"
    );
}

#[test]
fn backend_body_count() {
    // With monomorphization, only entry-point-reachable functions are emitted.
    // f1 and f2 are not called from main, so they are dead code and not
    // collected as mono items. Only main (1 body) reaches the backend.
    let (backend, result) = compile_with_mock(
        "
fn f1() {}
fn f2() {}
fn main() {}
",
    );
    assert!(result.is_ok());
    let calls = backend.calls();
    assert!(!calls.is_empty(), "no backend calls");
    let total_bodies: usize = calls.iter().map(|c| c.body_count).sum();
    // Only main is an entry point; f1/f2 are unreachable dead code.
    assert_eq!(total_bodies, 1, "expected 1 body (main only), got {}", total_bodies);
}
