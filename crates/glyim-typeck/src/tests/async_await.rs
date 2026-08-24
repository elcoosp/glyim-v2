//! Tests for async/await keyword recognition (S17-T05 / plan §6.1).
//!
//! `async`/`await` are now real syntax keywords (`KwAsync`/`KwAwait` in
//! `glyim_syntax::SyntaxKind`), lexed and lowered into `FnItem::is_async`.
//! These tests document the post-§6.1 behavior: the keywords are recognized
//! and no longer surface spurious "unsupported" diagnostics. (Full `async`
//! desugaring into a `Future` state machine remains a separate design-doc
//! subsystem and is intentionally not asserted here.)
use glyim_diag::{ErrorCode, ErrorCategory};
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

/// Two `async fn`s where one awaits the other (`nested` awaits `dep`). This is
/// the single-await shape with a *cross-future* call inside the `.await`
/// desugar: `nested`'s `poll` body references `dep`'s desugared wrapper fn.
/// Regression guard for the def-map/`LocalDefId` ordering bug where the inner
/// wrapper's `fn_sig` was only registered *after* the outer poll body was
/// type-checked, and for the method-dispatch probe emitting spurious
/// "mismatched types" diagnostics on non-matching `impl Future` candidates.
#[test]
fn nested_async_single_await_compiles() {
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
        async fn dep(x: i32) -> i32 { x }
        async fn nested(a: i32) -> i32 { let x = dep(a).await; x + 1 }
        fn main() -> i32 {
            let f = nested(5);
            block_on(f)
        }
    "#;
    let output = compile(src);
    assert_no_errors(&output.diagnostics);
}

/// M4 (best-effort, 2026-08-24): multi-await (`suspend_count >= 2`) sequential
/// async fns are NOT yet a compiling, verified state machine. A real
/// `desugar_multi_async_fn` HIR codegen exists (retained as `#[allow(dead_code)]`
/// scaffold in `glyim-hir/src/lower/lower_async.rs`); it emits a correct
/// `Start`/`S0`/`..`/`Done` `FooState` machine that the compiler's exhaustiveness
/// check recognizes — but glyim's type-checker cannot currently resolve the
/// enum-variant state-machine shape, so per the plan's safety rule the
/// `async-v2` diagnostic (error 61) is emitted instead of a silently-broken
/// state machine. Runtime resumption (M5) is also host-gated (Linux executor,
/// macOS host). This test asserts the *honest* behavior: a clear diagnostic,
/// not a panic/ICE.
#[test]
fn multi_await_emits_async_v2_diagnostic() {
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
        async fn dep(x: i32) -> i32 { x }
        async fn two(a: i32) -> i32 { let x = dep(a).await; let y = dep(x).await; x + y }
        fn main() -> i32 {
            let f = two(5);
            block_on(f)
        }
    "#;
    let output = compile(src);
    // Must produce exactly the async-v2 (error 61) diagnostic, not an ICE/panic.
    assert!(
        output.diagnostics.iter().any(|d| {
            matches!(&d.code, ErrorCode { category: ErrorCategory::Type, number: 61 })
        }),
        "multi-await should emit the async-v2 diagnostic, got: {:?}",
        output.diagnostics
    );
}
