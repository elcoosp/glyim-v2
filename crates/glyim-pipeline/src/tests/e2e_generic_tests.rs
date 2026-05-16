//! V33-T04: End-to-end compilation of generic library + binary.
//!
//! Tests the full pipeline with multiple functions, ensuring that
//! monomorphization, partitioning, borrow checking, optimization,
//! and codegen all work together correctly.

use crate::Pipeline;
use glyim_diag::CompResult;
use glyim_test::mock::{MockCodegen, TestDbBuilder};
use std::io::Write;
use std::sync::Arc;

fn compile_with_mock(source: &str) -> (MockCodegen, CompResult<()>) {
    let mut tmp = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    write!(tmp, "{}", source).expect("Failed to write temp file");
    let path = tmp.path().to_path_buf();
    let mut builder = TestDbBuilder::new()
        .name("test_e2e")
        .target_triple("x86_64-unknown-linux-gnu")
        .opt_level(0);
    builder = builder.file(path.clone(), Arc::from(source));
    let backend = MockCodegen::new();
    let mut db = builder.build();
    let output_path = std::path::Path::new("test_output.o");
    let result = Pipeline::compile_file(&mut db, &path, &backend, output_path);
    (backend, result)
}

#[test]
fn e2e_simple_main() {
    let (_, result) = compile_with_mock("fn main() {}");
    assert!(result.is_ok(), "simple main should compile: {:?}", result.err());
}

#[test]
fn e2e_multiple_functions() {
    let source = "
fn helper() {}
fn main() {}
";
    let (_, result) = compile_with_mock(source);
    assert!(result.is_ok(), "multiple functions should compile: {:?}", result.err());
}

#[test]
fn e2e_function_with_params() {
    let source = "fn add(x: i32, y: i32) -> i32 { x + y } fn main() {}";
    let (_, result) = compile_with_mock(source);
    assert!(result.is_ok(), "function with params should compile: {:?}", result.err());
}

#[test]
fn e2e_if_else_compiles() {
    let source = "fn main() { if true { 1 } else { 2 } }";
    let (_, result) = compile_with_mock(source);
    assert!(result.is_ok(), "if/else should compile: {:?}", result.err());
}

#[test]
fn e2e_syntax_error_fails_gracefully() {
    let (_, result) = compile_with_mock("fn main() {");
    assert!(result.is_err(), "syntax error should fail");
}

#[test]
fn e2e_type_error_fails_gracefully() {
    let (_, result) = compile_with_mock("fn main() { true + 1 }");
    assert!(result.is_err(), "type error should fail");
}

#[test]
fn e2e_backend_receives_bodies() {
    let (backend, result) = compile_with_mock("fn main() {}");
    assert!(result.is_ok());
    assert!(!backend.calls().is_empty(), "backend should receive bodies");
}

#[test]
fn e2e_backend_body_count_matches_functions() {
    // With monomorphization, only main is an entry point.
    // f1 and f2 are not reachable from main, so only 1 body is emitted.
    let source = "
fn f1() {}
fn f2() {}
fn main() {}
";
    let (backend, result) = compile_with_mock(source);
    assert!(result.is_ok());
    let total_bodies: usize = backend.calls().iter().map(|c| c.body_count).sum();
    assert_eq!(total_bodies, 1, "expected 1 body (main only), got {}", total_bodies);
}

#[test]
fn e2e_empty_file_compiles() {
    let (_, result) = compile_with_mock("");
    assert!(result.is_ok(), "empty file should compile: {:?}", result.err());
}

#[test]
fn e2e_nested_if_compiles() {
    let source = "fn main() { if true { if false { 1 } else { 2 } } else { 3 } }";
    let (_, result) = compile_with_mock(source);
    assert!(result.is_ok(), "nested if should compile: {:?}", result.err());
}

#[test]
fn e2e_while_loop_compiles() {
    let source = "fn main() { while true {} }";
    let (_, result) = compile_with_mock(source);
    assert!(result.is_ok(), "while loop should compile: {:?}", result.err());
}
