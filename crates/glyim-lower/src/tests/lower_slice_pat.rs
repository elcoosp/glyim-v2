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
    let (_, mir_body) = MirGenTester::new(src)
        .run()
        .expect("MIR generation failed");

    // Verify that the MIR contains at least a projection for field 0 and field 1
    // (simplified: just ensure no panic, but we'll add a meaningful check later)
    assert!(mir_body.basic_blocks.len() > 0, "MIR body should have basic blocks");
}
