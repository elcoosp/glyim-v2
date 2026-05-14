use crate::parser::parse_to_syntax;
use glyim_span::FileId;

fn parse_no_errors(source: &str) {
    let file_id = FileId::from_raw(0);
    let result = parse_to_syntax(source, file_id);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.is_error() || d.message.contains("STUB"))
        .collect();
    assert!(errors.is_empty(), "unexpected diagnostics: {:?}", errors);
}

#[test]
fn const_item_parses_cleanly() {
    parse_no_errors("const X: i32 = 42;");
}

#[test]
fn static_item_parses_cleanly() {
    parse_no_errors("static X: i32 = 42;");
}

#[test]
fn static_mut_item_parses_cleanly() {
    parse_no_errors("static mut X: i32 = 42;");
}

#[test]
fn type_alias_parses_cleanly() {
    parse_no_errors("type Foo = i32;");
}

#[test]
fn extern_block_parses_cleanly() {
    parse_no_errors("extern \"C\" { fn foo(); }");
}

#[test]
fn associated_type_in_trait_parses_cleanly() {
    parse_no_errors("trait T { type Foo; }");
}

#[test]
fn associated_const_in_trait_parses_cleanly() {
    parse_no_errors("trait T { const C: i32; }");
}

#[test]
fn associated_type_in_impl_parses_cleanly() {
    parse_no_errors("struct S; impl S { type Foo = i32; }");
}

#[test]
fn associated_const_in_impl_parses_cleanly() {
    parse_no_errors("struct S; impl S { const C: i32 = 42; }");
}

#[test]
fn raw_pointer_type_parses_cleanly() {
    parse_no_errors("type T = *const i32;");
    parse_no_errors("type T = *mut i32;");
}

#[test]
fn impl_trait_type_parses_cleanly() {
    parse_no_errors("fn foo() -> impl MyTrait { loop {} }");
}

#[test]
fn function_pointer_type_parses_cleanly() {
    parse_no_errors("type F = fn(i32) -> i32;");
}

#[test]
fn labeled_loop_parses_with_expected_errors() {
    // Labels are not yet lexed correctly; errors are expected.
    let file_id = FileId::from_raw(0);
    let result = parse_to_syntax("fn main() { 'outer: loop { break; } }", file_id);
    let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.is_error()).collect();
    assert!(
        !errors.is_empty(),
        "Expected parse errors for unsupported label syntax"
    );
}

#[test]
fn labeled_while_parses_with_expected_errors() {
    let file_id = FileId::from_raw(0);
    let result = parse_to_syntax("fn main() { 'lbl: while true { break; } }", file_id);
    assert!(result.diagnostics.iter().filter(|d| d.is_error()).count() > 0);
}

#[test]
fn labeled_for_parses_with_expected_errors() {
    let file_id = FileId::from_raw(0);
    let result = parse_to_syntax("fn main() { 'lbl: for _ in 0..1 { break; } }", file_id);
    assert!(result.diagnostics.iter().filter(|d| d.is_error()).count() > 0);
}

#[test]
fn labeled_block_parses_with_expected_errors() {
    let file_id = FileId::from_raw(0);
    let result = parse_to_syntax("fn main() { 'lbl: { break 'lbl; } }", file_id);
    assert!(result.diagnostics.iter().filter(|d| d.is_error()).count() > 0);
}
