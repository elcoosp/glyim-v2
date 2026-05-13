use crate::LlvmBackend;
use glyim_codegen::CodegenBackend;
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
    assert!(
        r2.is_ok(),
        "Second generate call should succeed (reuses Context)"
    );
}

#[test]
fn s08_t06_generate_function_with_body() {
    let backend = LlvmBackend::new();
    let body = std::sync::Arc::new(glyim_mir::Body::dummy(glyim_core::DefId::new(
        glyim_core::CrateId::from_raw(0),
        glyim_core::LocalDefId::from_raw(0),
    )));
    let result = backend.generate_function(&body);
    assert!(result.is_ok(), "generate_function should succeed");
    let bytes = result.unwrap();
    assert!(
        !bytes.is_empty(),
        "generate_function should return non-empty bytes"
    );
}

#[test]
fn s08_t07_with_target_constructor() {
    let backend = LlvmBackend::with_target("aarch64-unknown-linux-gnu");
    assert_eq!(backend.name(), "llvm");
    // Verify it doesn't crash on generate
    let output = std::path::Path::new("/tmp/glyim_test_aarch64.o");
    let bodies: Vec<std::sync::Arc<glyim_mir::Body>> = vec![];
    let result = backend.generate(&bodies, output);
    assert!(
        result.is_ok(),
        "generate with aarch64 target should succeed"
    );
}

#[test]
fn s08_t08_default_trait() {
    let backend: LlvmBackend = Default::default();
    assert_eq!(backend.name(), "llvm");
}

#[test]
fn s08_t09_generate_with_single_body() {
    let backend = LlvmBackend::new();
    let output = std::path::Path::new("/tmp/glyim_test_single.o");
    let body = std::sync::Arc::new(glyim_mir::Body::dummy(glyim_core::DefId::new(
        glyim_core::CrateId::from_raw(0),
        glyim_core::LocalDefId::from_raw(0),
    )));
    let bodies = vec![body];
    let result = backend.generate(&bodies, output);
    assert!(result.is_ok(), "generate with single body should succeed");
    // Verify output file was created
    assert!(output.exists(), "output file should exist after generate");
}

#[test]
fn s08_t10_generate_with_multiple_bodies() {
    let backend = LlvmBackend::new();
    let output = std::path::Path::new("/tmp/glyim_test_multi.o");
    let mut bodies: Vec<std::sync::Arc<glyim_mir::Body>> = vec![];
    for i in 0..5 {
        let body = std::sync::Arc::new(glyim_mir::Body::dummy(glyim_core::DefId::new(
            glyim_core::CrateId::from_raw(0),
            glyim_core::LocalDefId::from_raw(i),
        )));
        bodies.push(body);
    }
    let result = backend.generate(&bodies, output);
    assert!(result.is_ok(), "generate with 5 bodies should succeed");
    assert!(
        output.exists(),
        "output file should exist after generate with multiple bodies"
    );
}

#[test]
fn s08_t11_stress_many_bodies() {
    let backend = LlvmBackend::new();
    let output = std::path::Path::new("/tmp/glyim_test_stress.o");
    let mut bodies: Vec<std::sync::Arc<glyim_mir::Body>> = vec![];
    for i in 0..100 {
        let body = std::sync::Arc::new(glyim_mir::Body::dummy(glyim_core::DefId::new(
            glyim_core::CrateId::from_raw(0),
            glyim_core::LocalDefId::from_raw(i),
        )));
        bodies.push(body);
    }
    let result = backend.generate(&bodies, output);
    assert!(
        result.is_ok(),
        "generate with 100 bodies should succeed without crash"
    );
}

#[test]
fn s08_t12_generate_function_multiple_calls() {
    let backend = LlvmBackend::new();
    let body = std::sync::Arc::new(glyim_mir::Body::dummy(glyim_core::DefId::new(
        glyim_core::CrateId::from_raw(0),
        glyim_core::LocalDefId::from_raw(0),
    )));
    // Multiple generate_function calls should work (reuse context)
    let r1 = backend.generate_function(&body);
    let r2 = backend.generate_function(&body);
    let r3 = backend.generate_function(&body);
    assert!(r1.is_ok(), "First generate_function should succeed");
    assert!(r2.is_ok(), "Second generate_function should succeed");
    assert!(r3.is_ok(), "Third generate_function should succeed");
}

#[test]
fn s08_t13_empty_generate_function_returns_empty() {
    let backend = LlvmBackend::new();
    let body = std::sync::Arc::new(glyim_mir::Body::dummy(glyim_core::DefId::new(
        glyim_core::CrateId::from_raw(0),
        glyim_core::LocalDefId::from_raw(0),
    )));
    let result = backend.generate_function(&body);
    assert!(result.is_ok());
    // Empty body (no basic blocks) should still return valid bytes
    let bytes = result.unwrap();
    assert!(
        !bytes.is_empty(),
        "Even empty body should produce some bytes"
    );
}

#[test]
fn s08_t14_different_output_paths() {
    let backend = LlvmBackend::new();
    let bodies: Vec<std::sync::Arc<glyim_mir::Body>> = vec![];
    // Test with a path in a subdirectory (should fail gracefully, not panic)
    let output = std::path::Path::new("/tmp/glyim_nonexistent_dir/output.o");
    let result = backend.generate(&bodies, output);
    // Should return an error because directory doesn't exist
    assert!(
        result.is_err(),
        "generate to nonexistent directory should error"
    );
}
