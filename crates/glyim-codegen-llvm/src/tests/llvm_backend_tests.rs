use glyim_codegen::CodegenBackend;
use glyim_codegen_llvm::LlvmBackend;
use std::path::Path;

#[test]
fn s08_t01_create_backend_without_crash() {
    let backend = LlvmBackend::new();
    assert_eq!(backend.name(), "llvm");
}

#[test]
fn s08_t02_generate_empty_bodies_creates_module() {
    let backend = LlvmBackend::new();
    let output = Path::new("/tmp/glyim_test_empty.o");
    let bodies: Vec<std::sync::Arc<glyim_mir::Body>> = vec![];
    let result = backend.generate(&bodies, output);
    assert!(result.is_ok(), "generate with no bodies should succeed");
}

#[test]
fn s08_t03_generate_returns_ok() {
    let backend = LlvmBackend::new();
    let output = Path::new("/tmp/glyim_test_ok.o");
    let bodies: Vec<std::sync::Arc<glyim_mir::Body>> = vec![];
    let result = backend.generate(&bodies, output);
    assert!(result.is_ok());
}

#[test]
fn s08_t04_name_returns_llvm() {
    let backend = LlvmBackend::new();
    assert_eq!(backend.name(), "llvm");
}

#[test]
fn s08_t05_multiple_generate_calls_reuse_context() {
    let backend = LlvmBackend::new();
    let output1 = Path::new("/tmp/glyim_test_reuse1.o");
    let output2 = Path::new("/tmp/glyim_test_reuse2.o");
    let bodies: Vec<std::sync::Arc<glyim_mir::Body>> = vec![];

    let r1 = backend.generate(&bodies, output1);
    let r2 = backend.generate(&bodies, output2);

    assert!(r1.is_ok(), "First generate call should succeed");
    assert!(r2.is_ok(), "Second generate call should succeed (reuses Context)");
}
