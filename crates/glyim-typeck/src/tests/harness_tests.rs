use glyim_test::harness::{TestRunner, TestMode};

#[test]
fn compile_pass_tests() {
    TestRunner::new("tests/compile-pass")
        .mode(TestMode::CompilePass)
        .parallel(false)
        .build()
        .expect("failed to discover tests")
        .run();
}

#[test]
fn compile_fail_tests() {
    TestRunner::new("tests/compile-fail")
        .mode(TestMode::CompileFail)
        .parallel(false)
        .build()
        .expect("failed to discover tests")
        .run();
}
