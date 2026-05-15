use glyim_test::harness::{TestRunner, TestMode};
use std::path::PathBuf;

#[test]
fn run_pass_tests() {
    TestRunner::new(
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/test_data/run-pass"))
    )
    .mode(TestMode::CompilePass)
    .frontend_only()   // avoid LLVM dependency
    .build()
    .expect("failed to discover tests")
    .run();
}

#[test]
fn compile_fail_tests() {
    TestRunner::new(
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/test_data/compile-fail"))
    )
    .mode(TestMode::CompileFail)
    .frontend_only()
    .build()
    .expect("failed to discover tests")
    .run();
}
