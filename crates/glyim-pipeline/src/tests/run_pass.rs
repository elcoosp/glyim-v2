//! Run-pass tests for generic instantiation and drop glue (U02-T05, U02-T06)
use glyim_test::harness::TestRunner;
use glyim_test::harness::TestMode;

#[test]
fn generic_function_instantiation_works() {
    TestRunner::new("../glyim-worktrees/stream-U02/crates/glyim-pipeline/tests/compile-pass")
        .mode(TestMode::CompilePass)
        .filter("generic_fn")
        .parallel(false)
        .build()
        .expect("failed to discover tests")
        .run();
}

#[test]
fn drop_glue_generated_for_struct_with_box() {
    TestRunner::new("../glyim-worktrees/stream-U02/crates/glyim-pipeline/tests/compile-pass")
        .mode(TestMode::CompilePass)
        .filter("drop_glue")
        .parallel(false)
        .build()
        .expect("failed to discover tests")
        .run();
}
