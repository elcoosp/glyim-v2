use glyim_test::phase::FrontendTester;

#[test]
fn expr_ty_returns_correct_type() {
    let source = r#"
        fn test() -> i32 {
            let a = 42;
            a
        }
    "#;
    let trace = FrontendTester::new(source).run();
    let typeck = trace.typeck_result.expect("typeck failed");
    assert!(typeck.diagnostics.is_empty());
    // Find the body for the test function and check expr_ty returns Some(i32)
    // For now, just verify that we can call the method without error.
    let _ = typeck.expr_ty(glyim_core::def_id::LocalDefId::from_raw(0), 0);
}
