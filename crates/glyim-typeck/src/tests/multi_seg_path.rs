use glyim_test::phase::FrontendTester;

#[test]
fn multi_segment_type_path_resolves() {
    let source = r#"
        mod a {
            pub struct S;
        }
        fn test() {
            let x: a::S;
    "#;
    let trace = FrontendTester::new(source).run();
    let typeck = trace.typeck_result.expect("typeck failed");
    assert!(typeck.diagnostics.is_empty());
}
