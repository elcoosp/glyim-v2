use glyim_test::phase::FrontendTester;

#[test]
fn exhaustive_pattern_handling() {
    let source = r#"
        struct Point { x: i32, y: i32 }
        fn test(p: Point) {
            match p {
                Point { x, y } => { let _ = x + y; }
            }
        }
    "#;
    let trace = FrontendTester::new(source).run();
    let typeck = trace.typeck_result.expect("typeck failed");
    assert!(typeck.diagnostics.is_empty());
}
