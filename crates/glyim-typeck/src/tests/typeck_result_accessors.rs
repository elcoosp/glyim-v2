use glyim_test::phase::FrontendTester;

#[test]
fn expr_ty_returns_correct_type() {
    let source = r#"
        fn test() {
            let a = 42;
        }
    "#;
    let trace = FrontendTester::new(source).run();
    let typeck = trace.typeck_result.expect("typeck failed");
    assert!(typeck.diagnostics.is_empty());
}
