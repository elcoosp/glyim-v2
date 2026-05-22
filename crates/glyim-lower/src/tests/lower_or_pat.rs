use glyim_test::phase::MirGenTester;

#[test]
fn test_or_pattern_switch() {
    let src = r#"
        fn test(x: i32) -> bool {
            match x {
                0 | 1 => true,
                _ => false
            }
        }
    "#;
    let result = MirGenTester::from_source(src).run();
    assert!(result.is_ok(), "MIR generation failed");
    let (_, mir_body) = result.unwrap();

    let mut found_switch = false;
    for block in mir_body.basic_blocks.iter() {
        if let glyim_mir::TerminatorKind::SwitchInt { targets, .. } = &block.terminator.kind {
            let keys: Vec<_> = targets.iter().map(|(v,_)| v).collect();
            if keys.contains(&0) && keys.contains(&1) {
                found_switch = true;
                break;
            }
        }
    }
    assert!(found_switch, "Expected SwitchInt with keys 0 and 1 for OR pattern");
}
