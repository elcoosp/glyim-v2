use glyim_test::phase::MirGenTester;

#[test]
fn test_slice_pattern_binding() {
    let src = r#"
        fn test(arr: [i32; 3]) -> i32 {
            match arr {
                [a, b] => a + b,
                _ => 0
            }
        }
    "#;
    let result = MirGenTester::from_source(src).run();
    assert!(result.is_ok(), "MIR generation failed");
    let (_, mir_body) = result.unwrap();
    assert!(mir_body.basic_blocks.len() > 0, "MIR body should have basic blocks");
}
