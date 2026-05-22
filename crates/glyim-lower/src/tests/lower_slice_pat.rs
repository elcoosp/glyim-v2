use glyim_test::prelude::*;

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
    assert!(mir_body.basic_blocks.len() > 0, "MIR body should have basic blocks");
}
