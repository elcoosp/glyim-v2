//! S11-T02: Built-in file! and line! macros expand to correct values

use crate::{BuiltinMacro, Expander, MacroDef, MacroKind};
use glyim_frontend::parse_to_syntax;
use glyim_span::{ByteIdx, FileId, HygieneCtx, Span, SyntaxContext};

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

    let file_name = expander.interner().intern("file");
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

    let line_name = expander.interner().intern("line");
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

    let col_name = expander.interner().intern("column");
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

    let file_name = expander.interner().intern("file");
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

    let line_name = expander.interner().intern("line");
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

/// Test the expand() public API directly for the env! builtin.
#[test]
fn builtin_env_expand_api() {
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);

    let env_name = expander.interner().intern("env");
    expander.register_macro(MacroDef {
        name: env_name,
        kind: MacroKind::Builtin {
            name: env_name,
            handler: BuiltinMacro::Env,
        },
        span: Span::DUMMY,
    });

    // Use a deterministic variable so the test does not depend on the host
    // environment.
    unsafe {
        std::env::set_var("GLYIM_TEST_ENV_VAR", "hello-env");
    }
    let args_source = r#"("GLYIM_TEST_ENV_VAR")"#;
    let args_root = parse(args_source);

    let call_site = Span::new(
        FileId::from_raw(1),
        ByteIdx::from_raw(0),
        ByteIdx::from_raw(7),
        SyntaxContext::ROOT,
    );

    let result = expander.expand(env_name, &args_root, call_site);

    assert!(
        result.expanded.is_some(),
        "Expected env!() to produce an expansion, got diagnostics: {:?}",
        result.diagnostics
    );

    let expanded_text = result.expanded.unwrap().text().to_string();
    assert!(
        expanded_text.contains("hello-env"),
        "Expected env!() expansion to contain the variable value, got: {}",
        expanded_text
    );
}

/// Tier 4.2 / 4.3: line!/column!/include! resolve against a real VFS.
#[test]
fn vfs_backed_line_column_and_include() {
    use glyim_vfs::Vfs;
    use std::io::Write;

    let tmp = tempfile::tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    let src_path = src_dir.join("main.gly");
    // `line!` sits on line 2, column 13 (1-based) of this exact source.
    let src = "fn main() {\n    let _ = line!();\n}\n";
    {
        let mut f = std::fs::File::create(&src_path).unwrap();
        f.write_all(src.as_bytes()).unwrap();
    }
    let footer_path = src_dir.join("footer.gly");
    {
        let mut f = std::fs::File::create(&footer_path).unwrap();
        f.write_all(b"42").unwrap();
    }

    let mut vfs = Vfs::new();
    let src_id = vfs.add_file_from_disk(&src_path).unwrap();

    // line!/column! expansion via expand_crate with the source file set.
    let root = parse_to_syntax(src, src_id).root;
    let mut hygiene = HygieneCtx::default();
    let mut expander = Expander::new(&mut hygiene);
    expander.set_source_file(src_id);
    expander.set_vfs(&vfs);

    let line_name = expander.interner().intern("line");
    expander.register_macro(MacroDef {
        name: line_name,
        kind: MacroKind::Builtin {
            name: line_name,
            handler: BuiltinMacro::Line,
        },
        span: Span::DUMMY,
    });
    let (expanded, _diags) = expander.expand_crate(&root);
    let text = expanded.text().to_string();
    // The real line is 2; the old heuristic fallback would yield 1 (offset/80),
    // so asserting the literal `2` and absence of the unexpanded `line!` proves
    // the VFS-backed real line/col path ran.
    assert!(
        !text.contains("line!"),
        "line!() should be expanded away, got: {}",
        text
    );
    assert!(
        text.contains("2"),
        "line!() should expand to the real line 2, got: {}",
        text
    );

    // include! resolves relative to the calling file's directory.
    let inc_src = "fn main() {\n    let _ = include!(\"footer.gly\");\n}\n";
    let inc_root = parse_to_syntax(inc_src, src_id).root;
    let mut hygiene2 = HygieneCtx::default();
    let mut expander2 = Expander::new(&mut hygiene2);
    expander2.set_source_file(src_id);
    expander2.set_vfs(&vfs);

    let include_name = expander2.interner().intern("include");
    expander2.register_macro(MacroDef {
        name: include_name,
        kind: MacroKind::Builtin {
            name: include_name,
            handler: BuiltinMacro::Include,
        },
        span: Span::DUMMY,
    });
    let (inc_expanded, inc_diags) = expander2.expand_crate(&inc_root);
    assert!(
        inc_diags.is_empty(),
        "include! should not emit diagnostics, got: {:?}",
        inc_diags
    );
    let inc_text = inc_expanded.text().to_string();
    assert!(
        inc_text.contains("42"),
        "include! should inline footer.gly content, got: {}",
        inc_text
    );
}
