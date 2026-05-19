//! S11-T02: Built-in file! and line! macros expand to correct values

use crate::{Expander, MacroDef, MacroKind, BuiltinMacro};
use glyim_core::interner::Interner;
use glyim_diag::GlyimDiagnostic;
use glyim_span::{ByteIdx, FileId, HygieneCtx, Span, SyntaxContext};
use glyim_frontend::parse_to_syntax;

fn parse(source: &str) -> glyim_syntax::SyntaxNode {
    parse_to_syntax(source, FileId::BOGUS).root
}

/// Test that file!() expands to a string containing the file ID.
#[test]
fn file_macro_expands() {
    let source = r#"
fn main() {
    let _ = file!();
}
"#;
    let root = parse(source);
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);

    let interner = Interner::default();
    let file_name = interner.intern("file");
    expander.register_macro(MacroDef {
        name: file_name,
        kind: MacroKind::Builtin {
            name: file_name,
            handler: BuiltinMacro::File,
        },
        span: Span::DUMMY,
    });

    let (expanded, diags) = expander.expand_crate(&root);

    let expanded_text = expanded.text().to_string();
    assert!(
        !expanded_text.contains("file!()"),
        "Expected file!() to be expanded away, got: {}",
        expanded_text
    );
    // The expansion should contain a string literal
    assert!(
        expanded_text.contains('"'),
        "Expected file!() expansion to contain a string literal, got: {}",
        expanded_text
    );
    let _ = diags;
}

/// Test that line!() expands to a line number.
#[test]
fn line_macro_expands() {
    let source = r#"
fn main() {
    let _ = line!();
}
"#;
    let root = parse(source);
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);

    let interner = Interner::default();
    let line_name = interner.intern("line");
    expander.register_macro(MacroDef {
        name: line_name,
        kind: MacroKind::Builtin {
            name: line_name,
            handler: BuiltinMacro::Line,
        },
        span: Span::DUMMY,
    });

    let (expanded, diags) = expander.expand_crate(&root);

    let expanded_text = expanded.text().to_string();
    assert!(
        !expanded_text.contains("line!()"),
        "Expected line!() to be expanded away, got: {}",
        expanded_text
    );
    assert!(
        expanded_text.chars().any(|c: char| c.is_ascii_digit()),
        "Expected line!() expansion to contain a digit, got: {}",
        expanded_text
    );
    let _ = diags;
}

/// Test that column!() expands to a column number.
#[test]
fn column_macro_expands() {
    let source = r#"
fn main() {
    let _ = column!();
}
"#;
    let root = parse(source);
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);

    let interner = Interner::default();
    let col_name = interner.intern("column");
    expander.register_macro(MacroDef {
        name: col_name,
        kind: MacroKind::Builtin {
            name: col_name,
            handler: BuiltinMacro::Column,
        },
        span: Span::DUMMY,
    });

    let (expanded, diags) = expander.expand_crate(&root);

    let expanded_text = expanded.text().to_string();
    assert!(
        !expanded_text.contains("column!()"),
        "Expected column!() to be expanded away, got: {}",
        expanded_text
    );
    let _ = diags;
}

/// Test the expand() public API directly for the file! builtin.
#[test]
fn builtin_file_expand_api() {
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);

    let interner = Interner::default();
    let file_name = interner.intern("file");

    expander.register_macro(MacroDef {
        name: file_name,
        kind: MacroKind::Builtin {
            name: file_name,
            handler: BuiltinMacro::File,
        },
        span: Span::DUMMY,
    });

    let args_source = "()";
    let args_root = parse(args_source);

    let call_site = Span::new(
        FileId::from_raw(7),
        ByteIdx::from_raw(10),
        ByteIdx::from_raw(17),
        SyntaxContext::ROOT,
    );

    let result = expander.expand(file_name, &args_root, call_site);

    assert!(
        result.expanded.is_some(),
        "Expected file!() to produce an expansion, got diagnostics: {:?}",
        result.diagnostics
    );

    let expanded_text = result.expanded.unwrap().text().to_string();
    assert!(
        expanded_text.contains("7"),
        "Expected file!() expansion to reference file 7, got: {}",
        expanded_text
    );
}

/// Test the expand() public API directly for the line! builtin.
#[test]
fn builtin_line_expand_api() {
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);

    let interner = Interner::default();
    let line_name = interner.intern("line");

    expander.register_macro(MacroDef {
        name: line_name,
        kind: MacroKind::Builtin {
            name: line_name,
            handler: BuiltinMacro::Line,
        },
        span: Span::DUMMY,
    });

    let args_source = "()";
    let args_root = parse(args_source);

    let call_site = Span::new(
        FileId::from_raw(1),
        ByteIdx::from_raw(20),
        ByteIdx::from_raw(27),
        SyntaxContext::ROOT,
    );

    let result = expander.expand(line_name, &args_root, call_site);

    assert!(
        result.expanded.is_some(),
        "Expected line!() to produce an expansion, got diagnostics: {:?}",
        result.diagnostics
    );

    let expanded_text = result.expanded.unwrap().text().to_string();
    assert!(
        expanded_text.chars().any(|c: char| c.is_ascii_digit()),
        "Expected line!() expansion to contain a number, got: {}",
        expanded_text
    );
}

/// Test that env!() produces a diagnostic (not yet fully implemented).
#[test]
fn builtin_env_expand_api() {
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);

    let interner = Interner::default();
    let env_name = interner.intern("env");

    expander.register_macro(MacroDef {
        name: env_name,
        kind: MacroKind::Builtin {
            name: env_name,
            handler: BuiltinMacro::Env,
        },
        span: Span::DUMMY,
    });

    let args_source = r#"("PATH")"#;
    let args_root = parse(args_source);

    let call_site = Span::new(
        FileId::from_raw(1),
        ByteIdx::from_raw(0),
        ByteIdx::from_raw(7),
        SyntaxContext::ROOT,
    );

    let result = expander.expand(env_name, &args_root, call_site);

    // env! currently produces a diagnostic since it's not fully implemented
    assert!(
        !result.diagnostics.is_empty(),
        "Expected env!() to produce a diagnostic about not being implemented, got: {:?}",
        result.diagnostics
    );
    assert!(
        result.expanded.is_none(),
        "Expected env!() to not produce an expansion yet"
    );
}
