use glyim_test::prelude::*;

#[test]
fn test_index_projection() {
    let src = r#"
        fn test() -> i32 {
            let arr = [10, 20, 30];
            arr[1]
        }
    "#;
    let (ctx, mir_body) = MirGenTester::new(src)
        .run()
        .expect("MIR generation failed");

    let mut found_index = false;
    for block in mir_body.basic_blocks.iter() {
        for stmt in &block.statements {
            if let glyim_mir::StatementKind::Assign(place, _) = &stmt.kind {
                for proj in place.projection.iter() {
                    if matches!(proj, glyim_mir::ProjectionElem::Index(_)) {
                        found_index = true;
                    }
                }
            }
        }
    }
    assert!(found_index, "Expected Index projection in MIR");
}
