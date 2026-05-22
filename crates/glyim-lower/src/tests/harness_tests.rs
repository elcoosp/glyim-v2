use glyim_test::harness::{TestRunner, TestMode};
use glyim_test::CompilationTrace;

#[test]
#[ignore]
fn mir_feature_tests() {
    // Discover and run all .g files in tests/mir directory
    let plan = TestRunner::new("tests/mir")
        .mode(TestMode::CompilePass)
        .build()
        .expect("failed to discover tests");
    plan.run();
}
