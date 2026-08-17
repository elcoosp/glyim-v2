//! Tests for async/await keyword recognition (S17-T05 / plan §6.1).
//!
//! `async`/`await` are now real syntax keywords (`KwAsync`/`KwAwait` in
//! `glyim_syntax::SyntaxKind`), lexed and lowered into `FnItem::is_async`.
//! These tests document the post-§6.1 behavior: the keywords are recognized
//! and no longer surface spurious "unsupported" diagnostics. (Full `async`
//! desugaring into a `Future` state machine remains a separate design-doc
//! subsystem and is intentionally not asserted here.)
use glyim_test::phase::AnalysisTester;

#[test]
fn s17_t05_async_fn_recognized_without_error() {
    // `async fn` is now a recognized declaration form (lowered to
    // `FnItem { is_async: true }` by `lower_fn_def`); it must not produce
    // lex/parse/def-map diagnostics at this phase.
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
        all_diags.is_empty(),
        "async fn should be recognized without diagnostics, got: {:?}",
        all_diags
    );
}

#[test]
fn s17_t05_await_keyword_lexed_without_error() {
    // `await` is now a recognized keyword (`KwAwait`), so it must not produce
    // an "unknown identifier" lex error. (Parsing `.await` into a full await
    // expression is out of scope for plan §6.1 — only the keyword wiring is
    // implemented here.)
    let source = r#"
fn main() {
    let x = some_future;
    await;
}
"#;
    let result = AnalysisTester::new(source).run_def_map();
    assert!(
        result.lex_diagnostics.is_empty(),
        "await must be lexed as a keyword without errors, got: {:?}",
        result.lex_diagnostics
    );
}
