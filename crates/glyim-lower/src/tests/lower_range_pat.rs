use glyim_test::prelude::*;

#[test]
fn test_range_pattern_switch() {
    let src = r#"
        fn test(x: i32) -> bool {
            match x {
                0..=9 => true,
                _ => false
            }
        }
    "#;
    let (_, mir_body) = MirGenTester::new(src)
        .run()
        .expect("MIR generation failed");

    let mut found_switch = false;
    for block in mir_body.basic_blocks.iter() {
        if let glyim_mir::TerminatorKind::SwitchInt { targets, .. } = &block.terminator.kind {
            if targets.iter().count() >= 10 {
                found_switch = true;
                break;
            }
        }
    }
    assert!(found_switch, "Expected SwitchInt with at least 10 targets for 0..=9");
}
