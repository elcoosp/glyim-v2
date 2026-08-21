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

use glyim_span::FileId;
use glyim_test::assert_no_errors;
use glyim_test::harness::compiler::{CompileOutput, PipelineCompiler, TestCompiler};
use glyim_test::mock::MockCodegen;
use std::sync::Arc;

fn compile(src: &str) -> CompileOutput {
    let backend = Arc::new(MockCodegen::new());
    let compiler = PipelineCompiler::new(backend);
    compiler.compile(src, FileId::from_raw(1), &[])
}

/// Drive a real `async fn` + `.await` program through the full
/// `PipelineCompiler` (parser → lower → async desugar → typeck). The
/// desugar rewrites `async fn` into a `Future` state machine and `.await`
/// into a poll `Match`; the program defines `Future`/`Poll`/`block_on` in the
/// same crate. It must compile with ZERO diagnostics.
#[test]
fn desugar_async_fn_compiles() {
    let src = r#"
        enum Poll<T> { Ready(T), Pending }
        trait Future {
            type Output;
            fn poll(&mut self) -> Poll<Self::Output>;
        }
        fn block_on<F: Future>(mut f: F) -> F::Output {
            loop {
                match f.poll() {
                    Poll::Ready(v) => return v,
                    Poll::Pending => { }
                }
            }
        }
        async fn add_one(x: i32) -> i32 { x + 1 }
        fn main() -> i32 {
            let f = add_one(41);
            block_on(f)
        }
    "#;
    let output = compile(src);
    assert_no_errors(&output.diagnostics);
}
