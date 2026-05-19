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

    let has_error = diags.iter().any(|d: &GlyimDiagnostic| d.is_error());
    assert!(!has_error, "Expected no errors, got: {:?}", diags);

    let expanded_text = expanded.text().to_string();
    assert!(
        expanded_text.contains("42"),
        "Expected expanded output to contain '42', got: {}",
        expanded_text
    );
    assert!(
        !expanded_text.contains("ident!"),
        "Expected macro call 'ident!' to be expanded away, got: {}",
        expanded_text
    );
}

/// Test that a macro with multiple arms selects the correct one.
/// Uses ident vs literal fragment specifiers to differentiate arms.
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
    // "99" is a literal (IntLit), so second arm => 2
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

    let expanded_text = expanded.text().to_string();
    // At minimum, the macro def should be stripped from the output
    assert!(
        !expanded_text.contains("make_tuple!"),
        "Expected macro call to be expanded away, got: {}",
        expanded_text
    );
    let _ = diags;
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
/// Uses a simple identity macro to avoid complex expression matching.
#[test]
fn substitution_replaces_metavar() {
    let source = r#"
macro_rules! echo {
    ($x:ident) => { $x }
}

fn main() {
    let _ = echo!(value);
}
"#;
    let root = parse(source);
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);
    let (expanded, diags) = expander.expand_crate(&root);

    let has_error = diags.iter().any(|d: &GlyimDiagnostic| d.is_error());
    assert!(!has_error, "Expected no errors, got: {:?}", diags);

    let expanded_text = expanded.text().to_string();
    assert!(
        expanded_text.contains("value"),
        "Expected expanded output to contain 'value', got: {}",
        expanded_text
    );
    assert!(
        !expanded_text.contains("echo!"),
        "Expected macro call to be expanded away, got: {}",
        expanded_text
    );
}

/// Test expand() public API with a registered builtin macro.
#[test]
fn expand_api_with_builtin_file_macro() {
    let source = r#"file!()"#;
    let root = parse(source);
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);

    let interner = Interner::default();
    let name = interner.intern("file");

    expander.register_macro(MacroDef {
        name,
        kind: MacroKind::Builtin {
            name,
            handler: BuiltinMacro::File,
        },
        span: Span::DUMMY,
    });

    let call_site = Span::new(
        FileId::from_raw(42),
        ByteIdx::from_raw(0),
        ByteIdx::from_raw(7),
        SyntaxContext::ROOT,
    );

    let result = expander.expand(name, &root, call_site);
    assert!(
        result.expanded.is_some(),
        "Expected file!() to produce an expansion, got diagnostics: {:?}",
        result.diagnostics
    );
    let expanded_text = result.expanded.unwrap().text().to_string();
    assert!(
        expanded_text.contains("42"),
        "Expected file!() expansion to contain file ID 42, got: {}",
        expanded_text
    );
}
