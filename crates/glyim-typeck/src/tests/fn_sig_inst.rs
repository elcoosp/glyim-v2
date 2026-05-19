use glyim_test::phase::FrontendTester;

#[test]
fn function_signature_instantiation_substitutes_bound_vars() {
    let source = r#"
        fn foo(x: i32) -> i32 { x }
        fn test() {
            let y = foo(42);
        }
    "#;
    let trace = FrontendTester::new(source).run();
    let typeck = trace.typeck_result.expect("typeck failed");
    assert!(typeck.diagnostics.is_empty());
}
