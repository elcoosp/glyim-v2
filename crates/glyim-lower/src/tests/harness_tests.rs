use glyim_test::CompilationTrace;
use glyim_test::harness::{TestMode, TestRunner};

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
