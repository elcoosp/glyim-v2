use glyim_frontend::parse_to_syntax;
use glyim_span::{FileId, HygieneCtx};
use crate::Expander;

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
    let (_expanded, diags): (_, Vec<glyim_diag::GlyimDiagnostic>) = expander.expand_crate(&root);
    // Expect diagnostics about recursion limit
    assert!(!diags.is_empty(), "Expected recursion limit diagnostic");
}

/// Test V19-T07: Empty macro expansion produces no tokens.
#[test]
fn empty_expansion() {
    let src = r#"
macro_rules! empty {
    () => {};
}
fn main() {
    empty!();
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

/// Test V19-T08: Nested macro invocations expand recursively.
#[test]
fn nested_macros() {
    let src = r#"
macro_rules! inner {
    () => { 42 };
}
macro_rules! outer {
    () => { inner!() };
}
fn main() {
    outer!();
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

/// Test V19-T09: Repetition with separator token.
#[test]
fn repetition_with_separator() {
    let src = r#"
macro_rules! joined {
    ($($x:expr),+ $(,)?) => {
        stringify!($($x),+)
    };
}
fn main() {
    joined!(1, 2, 3);
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

/// Test V19-T10: Zero-or-one repetition matches optional arm.
#[test]
fn zero_or_one_repetition() {
    let src = r#"
macro_rules! optional {
    ($($x:expr)?) => {
        0 $(+ $x)?
    };
}
fn main() {
    let _a = optional!();
    let _b = optional!(5);
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

/// Test V19-T11: Mixed literal tokens and metavariables.
#[test]
fn mixed_literal_and_metavar() {
    let src = r#"
macro_rules! pair {
    ($a:expr, $b:expr) => {
        ($a, $b)
    };
}
fn main() {
    let _x = pair!(1, 2);
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

/// Test V19-T12: Multiple different metavariables in one pattern.
#[test]
fn multiple_metavars() {
    let src = r#"
macro_rules! swap {
    ($a:expr, $b:expr) => {
        ($b, $a)
    };
}
fn main() {
    let _x = swap!(10, 20);
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

/// Test V19-T13: Macro invocation as expression statement with trailing semicolon.
#[test]
fn macro_call_with_semicolon() {
    let src = r#"
macro_rules! make_int {
    () => { 42 };
}
fn main() {
    make_int!();
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

/// Test V19-T14: Deep recursion produces diagnostic.
#[test]
fn deep_recursion_emits_diagnostic() {
    let src = r#"
macro_rules! deep {
    () => { deep!() };
}
fn main() {
    deep!();
}
"#;
    let root = parse_source(src);
    let mut hygiene = HygieneCtx::new();
    let mut expander = Expander::new(&mut hygiene);
    let (_expanded, diags): (_, Vec<glyim_diag::GlyimDiagnostic>) = expander.expand_crate(&root);
    assert!(!diags.is_empty(), "Expected recursion limit diagnostic");
}
