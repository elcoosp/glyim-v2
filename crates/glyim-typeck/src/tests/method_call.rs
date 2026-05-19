use glyim_test::phase::FrontendTester;

#[test]
fn method_call_resolves_to_impl() {
    let source = r#"
        trait Foo {
            fn method(&self) -> i32;
        }
        impl Foo for i32 {
            fn method(&self) -> i32 { 42 }
        fn test() {
            let x = 5.method();
    "#;
    let trace = FrontendTester::new(source).run();
    let typeck = trace.typeck_result.expect("typeck failed");
    assert!(typeck.diagnostics.is_empty());
}
