use glyim_frontend::parse_to_syntax;
use glyim_span::{FileId, HygieneCtx};
use crate::{Expander, ExpansionResult};

/// Helper to parse a source string and get the root syntax node.
fn parse_source(source: &str) -> glyim_syntax::SyntaxNode {
    let result = parse_to_syntax(source, FileId::BOGUS);
    assert!(result.diagnostics.is_empty(), "Unexpected parse errors: {:?}", result.diagnostics);
    result.root
}

/// Helper to count the number of macro call nodes in a syntax tree.
fn count_macro_calls(node: &glyim_syntax::SyntaxNode) -> usize {
    use glyim_syntax::SyntaxKind;
    let mut count = 0;
    if node.kind() == SyntaxKind::MacroCall {
        count += 1;
    }
    for child in node.children() {
        count += count_macro_calls(&child);
    }
    count
}

/// Test V19-T01: Simple macro without arguments expands to literal.
#[test]
fn simple_macro_expands() {
    let src = r#"
macro_rules! foo {
    () => { 42 };
}
fn main() {
    foo!();
}
"#;
    let root = parse_source(src);
    let mut hygiene = HygieneCtx::new();
    let mut expander = Expander::new(&mut hygiene);
    let (expanded, diags): (_, Vec<glyim_diag::GlyimDiagnostic>) = expander.expand_crate(&root);
    assert!(diags.is_empty(), "Diagnostics: {:?}", diags);
    let remaining_macros = count_macro_calls(&expanded);
    assert_eq!(remaining_macros, 0, "Expected all macros expanded");
}

/// Test V19-T02: Macro with repetition $(...)* expands.
#[test]
fn repetition_macro_expands() {
    let src = r#"
macro_rules! vec_like {
    ($($x:expr),* $(,)?) => {
        {
            let mut v = Vec::new();
            $( v.push($x); )*
            v
        }
    };
}
fn main() {
    vec_like!(1, 2, 3);
}
"#;
    let root = parse_source(src);
    let mut hygiene = HygieneCtx::new();
    let mut expander = Expander::new(&mut hygiene);
    let (expanded, diags): (_, Vec<glyim_diag::GlyimDiagnostic>) = expander.expand_crate(&root);
    assert!(diags.is_empty(), "Diagnostics: {:?}", diags);
    let remaining_macros = count_macro_calls(&expanded);
    assert_eq!(remaining_macros, 0, "Expected all macros expanded");
}

/// Test V19-T03: Hygiene – macro-introduced variables don't leak.
#[test]
fn hygiene_variables_dont_leak() {
    let src = r#"
macro_rules! make_local {
    () => { let x = 5; };
}
fn main() {
    let x = 10;
    make_local!();
    x
}
"#;
    let root = parse_source(src);
    let mut hygiene = HygieneCtx::new();
    let mut expander = Expander::new(&mut hygiene);
    let (expanded, diags): (_, Vec<glyim_diag::GlyimDiagnostic>) = expander.expand_crate(&root);
    assert!(diags.is_empty(), "Diagnostics: {:?}", diags);
    let remaining_macros = count_macro_calls(&expanded);
    assert_eq!(remaining_macros, 0);
}

/// Test V19-T04: $crate resolves to crate path.
#[test]
fn dollar_crate_resolves() {
    let src = r#"
macro_rules! call_from_crate {
    () => { $crate::some_func(); };
}
fn main() {
    call_from_crate!();
}
"#;
    let root = parse_source(src);
    let mut hygiene = HygieneCtx::new();
    let mut expander = Expander::new(&mut hygiene);
    let (expanded, diags): (_, Vec<glyim_diag::GlyimDiagnostic>) = expander.expand_crate(&root);
    assert!(diags.is_empty(), "Diagnostics: {:?}", diags);
    let remaining_macros = count_macro_calls(&expanded);
    assert_eq!(remaining_macros, 0);
}

/// Test V19-T05: Macro with multiple arms (pattern matching).
#[test]
fn multiple_arms() {
    let src = r#"
macro_rules! choose {
    (a) => { 1 };
    (b) => { 2 };
}
fn main() {
    let _x = choose!(a);
    let _y = choose!(b);
}
"#;
    let root = parse_source(src);
    let mut hygiene = HygieneCtx::new();
    let mut expander = Expander::new(&mut hygiene);
    let (expanded, diags): (_, Vec<glyim_diag::GlyimDiagnostic>) = expander.expand_crate(&root);
    assert!(diags.is_empty(), "Diagnostics: {:?}", diags);
    let remaining_macros = count_macro_calls(&expanded);
    assert_eq!(remaining_macros, 0);
}

/// Test V19-T06: Macro recursion limit.
#[test]
fn recursion_limit_hits() {
    let src = r#"
macro_rules! recurse {
    ($x:expr) => { recurse!($x) };
}
fn main() {
    recurse!(0);
}
"#;
    let root = parse_source(src);
    let mut hygiene = HygieneCtx::new();
    let mut expander = Expander::new(&mut hygiene);
    let (expanded, diags): (_, Vec<glyim_diag::GlyimDiagnostic>) = expander.expand_crate(&root);
    // Expect diagnostics about recursion limit
    assert!(!diags.is_empty(), "Expected recursion limit diagnostic");
}
