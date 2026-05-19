use glyim_test::harness::{TestMode, TestRunner};
use std::path::PathBuf;

#[test]
#[ignore = "projection syntax not yet in parser"]
fn run_pass_tests() {
    TestRunner::new(PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_data/run-pass"
    )))
    .mode(TestMode::CompilePass)
    .frontend_only() // avoid LLVM dependency
    .build()
    .expect("failed to discover tests")
    .run();
}
fn compile_fail_tests() {
        "/test_data/compile-fail"
    .mode(TestMode::CompileFail)
    .frontend_only()
