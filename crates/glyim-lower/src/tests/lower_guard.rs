use glyim_test::prelude::*;

#[test]
fn test_match_guard() {
    let src = r#"
        fn test(x: Option<i32>) -> i32 {
            match x {
                Some(y) if y > 0 => y,
                _ => 0
            }
        }
    "#;
    let (_, mir_body) = MirGenTester::new(src)
        .run()
        .expect("MIR generation failed");

    let mut found_guard = false;
    for block in mir_body.basic_blocks.iter() {
        if let glyim_mir::TerminatorKind::SwitchInt { .. } = &block.terminator.kind {
            found_guard = true;
            break;
        }
    }
    assert!(found_guard, "Expected a guard branch in MIR");
}
