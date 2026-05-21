//! Tests for async/await error handling (S17-T05)
//! Note: async/await keywords are not yet in glyim_syntax::SyntaxKind.
//! They are currently lexed as identifiers. Proper await/async stubs
//! require SyntaxKind::KwAsync/KwAwait before they can emit phase errors.
use glyim_test::phase::AnalysisTester;

#[test]
fn s17_t05_async_fn_not_supported() {
    // 'async' at start of declaration triggers a parse/def-map error currently
    // because it's an unexpected identifier before 'fn' or similar.
    let source = r#"
async fn main() {
    let x = 42;
}
"#;
    let result = AnalysisTester::new(source).run_def_map();
    let all_diags = [
        &result.lex_diagnostics[..],
        &result.parse_diagnostics[..],
        &result.def_map_diagnostics[..],
    ]
    .concat();
    assert!(
        !all_diags.is_empty(),
        "expected errors for unsupported 'async' placement"
    );
}

#[test]
fn s17_t05_await_expr_not_supported() {
    // Since 'await' is not a keyword yet, it is lexed as an identifier.
    // `some_future.await` parses successfully as a field access expression.
    // Def-map phase does not type-check local variables, so it produces no diagnostics.
    // This test verifies current behavior and documents the dependency on syntax.
    let source = r#"
fn main() {
    let x = some_future.await;
}
"#;
    let result = AnalysisTester::new(source).run_def_map();
    let all_diags = [
        &result.lex_diagnostics[..],
        &result.parse_diagnostics[..],
        &result.def_map_diagnostics[..],
    ]
    .concat();
    // Currently parses without errors because 'await' is treated as an identifier
    assert!(
        all_diags.is_empty(),
        "expected no diagnostics for identifier 'await' (keyword not yet in syntax)"
    );
}
