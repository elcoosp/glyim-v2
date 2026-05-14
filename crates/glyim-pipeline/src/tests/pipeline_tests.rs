use glyim_diag::CompResult;
use glyim_test::mock::{MockCodegen, TestDbBuilder};
use std::path::PathBuf;
use std::sync::Arc;

use crate::Pipeline;

/// Helper to create a DB with a file and return its path.
fn setup_db(source: &str, file_name: &str) -> (TestDbBuilder, PathBuf) {
    let mut builder = TestDbBuilder::new()
        .name("test_pipeline")
        .target_triple("x86_64-unknown-linux-gnu")
        .opt_level(0);
    let path = PathBuf::from(file_name);
    builder = builder.file(path.clone(), Arc::from(source));
    (builder, path)
}

fn compile_with_mock(source: &str, file_name: &str) -> (MockCodegen, CompResult<()>) {
    let (builder, path) = setup_db(source, file_name);
    let backend = MockCodegen::new();
    let mut db = builder.build();
    let result = Pipeline::compile_file(&mut db, &path, &backend);
    (backend, result)
}

// S18-T01: Compile empty file -> Ok
#[test]
fn compile_empty_file_ok() {
    let (_, result) = compile_with_mock("", "empty.g");
    assert!(result.is_ok(), "expected Ok, got {:?}", result.err());
}

// S18-T02: Compile fn main() {} -> Ok
#[test]
fn compile_simple_main_ok() {
    let (_, result) = compile_with_mock("fn main() {}", "main.g");
    assert!(result.is_ok(), "expected Ok, got {:?}", result.err());
}

// S18-T03: Compile type error -> diagnostic
#[test]
fn compile_type_error_produces_diagnostics() {
    let (_, result) =
        compile_with_mock("fn main() { let x: i32 = \"hello\"; }", "type_err.g");
    match result {
        Err(diags) => {
            assert!(!diags.is_empty(), "expected diagnostics on type error");
            // Optional: check that at least one is a type error
            assert!(
                diags.iter().any(|d| d.is_error()),
                "should contain at least one error diagnostic"
            );
        }
        Ok(_) => panic!("expected compilation to fail with errors"),
    }
}

// S18-T04: Compile syntax error -> diagnostic
#[test]
fn compile_syntax_error_produces_diagnostics() {
    let (_, result) = compile_with_mock("fn main() {", "syntax_err.g");
    match result {
        Err(diags) => {
            assert!(!diags.is_empty());
            assert!(diags.iter().any(|d| d.is_error()));
        }
        Ok(_) => panic!("expected compilation to fail"),
    }
}

// S18-T05: Missing file -> I/O error
#[test]
fn compile_missing_file_returns_io_error() {
    let backend = MockCodegen::new();
    let mut db = TestDbBuilder::new()
        .name("test_missing")
        .build();
    let path = PathBuf::from("nonexistent.g");
    let result = Pipeline::compile_file(&mut db, &path, &backend);
    match result {
        Err(diags) => {
            assert!(!diags.is_empty());
            // Should mention I/O
            assert!(diags.iter().any(|d| d.message.contains("I/O") || d.message.to_lowercase().contains("io")));
        }
        Ok(_) => panic!("expected failure"),
    }
}

// S18-T06: Backend selection (backend.generate is called)
#[test]
fn backend_generate_is_called() {
    let (backend, result) = compile_with_mock("fn main() {}", "main.g");
    assert!(result.is_ok());
    // MockCodegen should have been called at least once
    assert!(backend.function_call_count() > 0, "backend.generate was not invoked");
}
