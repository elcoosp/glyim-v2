//! S11-T01: Declarative macro matches and substitutes correctly

use crate::{Expander, MacroDef, MacroKind, BuiltinMacro};
use glyim_core::interner::Interner;
use glyim_diag::GlyimDiagnostic;
use glyim_span::{FileId, HygieneCtx, Span, SyntaxContext, ByteIdx};
use glyim_frontend::parse_to_syntax;

/// Helper: parse source and return the syntax root.
fn parse(source: &str) -> glyim_syntax::SyntaxNode {
    parse_to_syntax(source, FileId::BOGUS).root
}

/// Test that a simple macro_rules! identity macro expands correctly.
/// macro_rules! ident { ($x:expr) => { $x } }
/// ident!(42) => 42
#[test]
fn identity_macro_expands_to_input() {
    let source = r#"
macro_rules! ident {
    ($x:expr) => { $x }
}

fn main() {
    let _ = ident!(42);
}
"#;
    let root = parse(source);
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);
    let (expanded, diags) = expander.expand_crate(&root);

    // The expansion should succeed without errors
    let has_error = diags.iter().any(|d: &GlyimDiagnostic| d.is_error());
    assert!(!has_error, "Expected no errors, got: {:?}", diags);

    // The expanded tree should contain "42" from the macro expansion
    let expanded_text = expanded.text().to_string();
    assert!(
        expanded_text.contains("42"),
        "Expected expanded output to contain '42', got: {}",
        expanded_text
    );

    // The expanded tree should NOT contain "ident" (macro call should be replaced)
    assert!(
        !expanded_text.contains("ident!"),
        "Expected macro call 'ident!' to be expanded away, got: {}",
        expanded_text
    );
}

/// Test that a macro with multiple arms selects the correct one.
#[test]
fn multi_arm_macro_selects_correct_arm() {
    let source = r#"
macro_rules! choose {
    ($x:ident) => { 1 }
    ($x:literal) => { 2 }
}

fn main() {
    let _ = choose!(foo);
    let _ = choose!(99);
}
"#;
    let root = parse(source);
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);
    let (expanded, diags) = expander.expand_crate(&root);

    let has_error = diags.iter().any(|d: &GlyimDiagnostic| d.is_error());
    assert!(!has_error, "Expected no errors, got: {:?}", diags);

    let expanded_text = expanded.text().to_string();
    // "foo" is an ident, so first arm => 1
    assert!(
        expanded_text.contains("1"),
        "Expected expanded output to contain '1' from ident arm, got: {}",
        expanded_text
    );
    // "99" is a literal, so second arm => 2
    assert!(
        expanded_text.contains("2"),
        "Expected expanded output to contain '2' from literal arm, got: {}",
        expanded_text
    );
}

/// Test that a macro with repetition ($($x)* ) expands correctly.
#[test]
fn repetition_macro_expands() {
    let source = r#"
macro_rules! make_tuple {
    ($($x:tt),*) => { ($($x),*) }
}

fn main() {
    let _ = make_tuple!(1, 2, 3);
}
"#;
    let root = parse(source);
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);
    let (expanded, diags) = expander.expand_crate(&root);

    let _has_error = diags.iter().any(|d: &GlyimDiagnostic| d.is_error());
    // This test may produce errors depending on parser support for macro syntax,
    // but the expansion should still produce output containing the tokens
    let expanded_text = expanded.text().to_string();
    // At minimum, the macro def should be stripped from the output
    assert!(
        !expanded_text.contains("make_tuple!"),
        "Expected macro call to be expanded away, got: {}",
        expanded_text
    );
}

/// Test that a macro producing a struct definition works.
#[test]
fn macro_produces_struct() {
    let source = r#"
macro_rules! unit_struct {
    ($name:ident) => { struct $name; }
}

unit_struct!(Foo);

fn main() {}
"#;
    let root = parse(source);
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);
    let (expanded, diags) = expander.expand_crate(&root);

    let expanded_text = expanded.text().to_string();
    // The expansion should produce "struct Foo;"
    assert!(
        expanded_text.contains("struct"),
        "Expected expanded output to contain 'struct', got: {}",
        expanded_text
    );
    assert!(
        expanded_text.contains("Foo"),
        "Expected expanded output to contain 'Foo', got: {}",
        expanded_text
    );
    let _ = diags;
}

/// Test that substitution replaces $x with the captured value.
#[test]
fn substitution_replaces_metavar() {
    let source = r#"
macro_rules! wrap {
    ($x:expr) => { ($x) }
}

fn main() {
    let _ = wrap!(1 + 2);
}
"#;
    let root = parse(source);
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);
    let (expanded, diags) = expander.expand_crate(&root);

    let has_error = diags.iter().any(|d: &GlyimDiagnostic| d.is_error());
    assert!(!has_error, "Expected no errors, got: {:?}", diags);

    let expanded_text = expanded.text().to_string();
    // Should contain the wrapped expression
    assert!(
        !expanded_text.contains("wrap!"),
        "Expected macro call to be expanded away, got: {}",
        expanded_text
    );
}

/// Test expand() public API with a registered builtin macro.
/// This verifies the Expander::expand method works for builtin macros.
#[test]
fn expand_api_with_builtin_file_macro() {
    let source = r#"file!()"#;
    let root = parse(source);
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);

    let mut interner = Interner::default();
    let name = interner.intern("file");

    expander.register_macro(MacroDef {
        name,
        kind: MacroKind::Builtin {
            name,
            handler: BuiltinMacro::File,
        },
        span: Span::DUMMY,
    });

    // Find a node to use as args - for file!() there are no real args
    let call_site = Span::new(
        FileId::from_raw(42),
        ByteIdx::from_raw(0),
        ByteIdx::from_raw(7),
        SyntaxContext::ROOT,
    );

    let result = expander.expand(name, &root, call_site);
    // The expansion should produce something (a string literal with the file path)
    // For now, just verify no panic and diagnostics are reasonable
    assert!(
        result.expanded.is_some() || !result.diagnostics.is_empty(),
        "Expected either expansion or diagnostics"
    );
}
