use glyim_test::phase::MirGenTester;

#[test]
fn test_struct_spread() {
    let src = r#"
        struct Point { x: i32, y: i32 }
        fn test(base: Point) -> Point {
            Point { x: 1, ..base }
        }
    "#;
    let result = MirGenTester::from_source(src).run();
    assert!(result.is_ok(), "MIR generation failed");
    let (_, mir_body) = result.unwrap();

    let mut found_aggregate = false;
    for block in mir_body.basic_blocks.iter() {
        for stmt in &block.statements {
            if let glyim_mir::StatementKind::Assign(_, rvalue) = &stmt.kind {
                if let glyim_mir::Rvalue::Aggregate(_, ops) = rvalue {
                    if ops.len() >= 2 {
                        found_aggregate = true;
                    }
                }
            }
        }
    }
    assert!(found_aggregate, "Expected Aggregate Rvalue for struct creation with spread");
}
