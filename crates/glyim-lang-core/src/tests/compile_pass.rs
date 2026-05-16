//! Tests that core library .g files can be lexed and parsed by the Glyim frontend.

use glyim_frontend::{lex, parse_to_syntax};
use glyim_span::{FileId, Span};
use glyim_lang_core::core_source;

fn file_id() -> FileId {
    FileId::from_raw(99)
}

/// Helper: lex a core module source and assert no errors.
fn lex_module(name: &str) {
    let source = core_source(name).unwrap_or_else(|| panic!("no source for module '{}'", name));
    let result = lex(source, file_id());
    assert!(
        result.diagnostics.is_empty(),
        "lex errors in module '{}': {:?}",
        name,
        result.diagnostics
    );
}

/// Helper: parse a core module source and assert no errors.
fn parse_module(name: &str) {
    let source = core_source(name).unwrap_or_else(|| panic!("no source for module '{}'", name));
    let result = parse_to_syntax(source, file_id());
    assert!(
        result.diagnostics.is_empty(),
        "parse errors in module '{}': {:?}",
        name,
        result.diagnostics
    );
}

// V16-T01: Option module lexes and parses
#[test]
fn t01_option_lex() {
    lex_module("option");
}

#[test]
fn t01_option_parse() {
    parse_module("option");
}

// V16-T02: Iter module lexes and parses (IntoIterator for slice)
#[test]
fn t02_iter_lex() {
    lex_module("iter");
}

#[test]
fn t02_iter_parse() {
    parse_module("iter");
}

// V16-T03: Panic module lexes and parses
#[test]
fn t03_panic_lex() {
    lex_module("panic");
}

#[test]
fn t03_panic_parse() {
    parse_module("panic");
}

// V16-T04: Mem module lexes and parses (mem::replace, mem::swap)
#[test]
fn t04_mem_lex() {
    lex_module("mem");
}

#[test]
fn t04_mem_parse() {
    parse_module("mem");
}

// V16-T05: Cmp module lexes and parses (cmp::min, cmp::max)
#[test]
fn t05_cmp_lex() {
    lex_module("cmp");
}

#[test]
fn t05_cmp_parse() {
    parse_module("cmp");
}

// V16-T06: Default module lexes and parses
#[test]
fn t06_default_lex() {
    lex_module("default");
}

#[test]
fn t06_default_parse() {
    parse_module("default");
}

// Additional module lex/parse tests
#[test]
fn result_lex() { lex_module("result"); }
#[test]
fn result_parse() { parse_module("result"); }

#[test]
fn slice_lex() { lex_module("slice"); }
#[test]
fn slice_parse() { parse_module("slice"); }

#[test]
fn str_lex() { lex_module("str"); }
#[test]
fn str_parse() { parse_module("str"); }

#[test]
fn cell_lex() { lex_module("cell"); }
#[test]
fn cell_parse() { parse_module("cell"); }

#[test]
fn ptr_lex() { lex_module("ptr"); }
#[test]
fn ptr_parse() { parse_module("ptr"); }

#[test]
fn ops_lex() { lex_module("ops"); }
#[test]
fn ops_parse() { parse_module("ops"); }

#[test]
fn marker_lex() { lex_module("marker"); }
#[test]
fn marker_parse() { parse_module("marker"); }

#[test]
fn hint_lex() { lex_module("hint"); }
#[test]
fn hint_parse() { parse_module("hint"); }

#[test]
fn convert_lex() { lex_module("convert"); }
#[test]
fn convert_parse() { parse_module("convert"); }
