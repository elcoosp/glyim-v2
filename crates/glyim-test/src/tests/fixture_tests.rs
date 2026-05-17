use crate::*;

#[test]
fn test_source_builder() {
    let source = fixtures::SourceBuilder::new()
        .mode("compile-fail")
        .empty()
        .fn_def("main", "", r#"let x: i32 = "hello""#)
        .annotation("ERROR mismatched types")
        .build();
    assert!(source.contains("fn main"));
    assert!(source.contains("//~ ERROR"));
    assert!(source.contains("// test-mode: compile-fail"));
}

#[test]
fn test_source_builder_empty() {
    let source = fixtures::SourceBuilder::new().build();
    assert!(source.is_empty());
}
