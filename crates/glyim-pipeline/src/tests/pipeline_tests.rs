use glyim_diag::CompResult;
use glyim_test::mock::{MockCodegen, TestDbBuilder};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::NamedTempFile;

use crate::Pipeline;

/// Helper: create a temporary file with the given source, and a TestDbBuilder with that file.
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
    let result = Pipeline::compile_file(&mut db, &path, &backend);
    (backend, result)
}

// S18-T01: Compile empty file -> Ok
#[test]
fn compile_empty_file_ok() {
    let (_, result) = compile_with_mock("");
    assert!(result.is_ok(), "expected Ok, got {:?}", result.err());
}

// S18-T02: Compile fn main() {} -> Ok
#[test]
fn compile_simple_main_ok() {
    let (_, result) = compile_with_mock("fn main() {}");
    assert!(result.is_ok(), "expected Ok, got {:?}", result.err());
}

// S18-T03: Compile type error (binary with mismatched types) -> diagnostic
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

// S18-T04: Compile syntax error -> diagnostic
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

// S18-T05: Missing file -> I/O error
#[test]
fn compile_missing_file_returns_io_error() {
    let backend = MockCodegen::new();
    let mut db = TestDbBuilder::new().name("test_missing").build();
    let path = PathBuf::from("nonexistent_xyz123.g");
    let result = Pipeline::compile_file(&mut db, &path, &backend);
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

// S18-T06: Backend selection (backend.generate is called)
#[test]
fn backend_generate_is_called() {
    let (backend, result) = compile_with_mock("fn main() {}");
    assert!(result.is_ok());
    assert!(
        !backend.calls().is_empty(),
        "backend.generate was not invoked"
    );
}
