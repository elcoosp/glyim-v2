use crate::parse_to_syntax;
use glyim_span::FileId;

#[test]
fn test_parse_fn() {
    let source = r#"
        fn main() {
            let x = 1;
        }
    "#;
    let result = parse_to_syntax(source, FileId::from_raw(0));
    assert!(
        result.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        result.diagnostics
    );
    assert_eq!(result.root.kind(), glyim_syntax::SyntaxKind::SourceFile);
}

#[test]
fn test_parse_struct() {
    let source = r#"
        struct Point {
            x: i32,
            y: i32,
        }
    "#;
    let result = parse_to_syntax(source, FileId::from_raw(0));
    assert!(
        result.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        result.diagnostics
    );
}

#[test]
fn test_parse_expr() {
    let source = r#"
        fn main() {
            let x = 1 + 2 * 3;
        }
    "#;
    let result = parse_to_syntax(source, FileId::from_raw(0));
    assert!(
        result.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        result.diagnostics
    );
}

#[test]
fn test_parse_if_let() {
    let source = r#"
        fn main() {
            let x = Some(1);
            if let Some(y) = x {
                println!("{}", y);
            }
        }
    "#;
    let result = parse_to_syntax(source, FileId::from_raw(0));
    assert!(
        result.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        result.diagnostics
    );
}

#[test]
fn test_parse_macro_def() {
    let source = r#"
        macro_rules! foo {
            () => {}
        }
    "#;
    let result = parse_to_syntax(source, FileId::from_raw(0));
    assert!(
        result.diagnostics.is_empty(),
        "Parsing failed: {:?}",
        result.diagnostics
    );
}
