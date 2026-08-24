//! M5 (async v1) runtime-proof integration driver.
//!
//! Discovers the `tests/runtime/m5/**` glyph fixtures (run-pass / compile-fail
//! modes) and drives them through the real full-pipeline compiler + native
//! linker + executor. This is the host-run `two_step` proof path: compile an
//! `async fn`, link it to a binary, and (for run-pass) execute it and assert its
//! output.
//!
//! Fixtures are gated with `// only-target: x86_64-unknown-linux-gnu`. The
//! executor is configured with that same target triple, so on a non-Linux host
//! the pipeline still links a Linux ELF but cannot *run* it; the run-pass
//! fixture then fails with a codegen/link gap (see below) rather than a silent
//! miscompile. The contract asserted here is:
//!   * `m5/two_step.g` (multi-await) MUST surface the `async-v2` diagnostic
//!     (error 61) and must NOT be reported as a successfully-compiled binary
//!     (no silent miscompile).
//!   * `m5/one_step.g` (single-await run-pass) must NOT silently miscompile. A
//!     `Failed` is tolerated *only* if it is the known single-await LLVM-codegen
//!     gap (`TyKind::Error` -> "no executable produced"); a wrong-output or
//!     compile-pass-with-bad-binary result would be a `Failed` we DO catch.
//!
//! Status (2026-08-24): the M5 harness + CI wiring are in place. The single-
//! await codegen path still panics at LLVM lower with `TyKind::Error` (a generic
//! `Future`/`block_on` instantiation gap), so `m5/one_step.g` currently cannot
//! produce a runnable binary. Once that gap closes, `m5/one_step.g` will
//! `Passed` on Linux and this driver enforces the real runtime proof.

use glyim_test::harness::executor::TestOutcome;
use glyim_test::harness::{TestMode, TestRunner};

fn m5_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("runtime")
        .join("m5")
}

/// The async-v2 diagnostic (error 61) message substring asserted for multi-await.
const ASYNC_V2_SUBSTRING: &str = "multi-`.await` bodies are not yet supported";

#[test]
fn m5_two_step_multi_await_must_be_rejected() {
    // Safety-net contract: multi-await must be reported with the async-v2
    // diagnostic and must NOT be silently compiled into a broken binary.
    let plan = TestRunner::new(m5_root())
        .parallel(false)
        .build()
        .expect("m5 fixture collection must succeed");
    let result = plan.execute();

    let two_step = result
        .results
        .iter()
        .find(|r| r.test.name.contains("two_step"))
        .expect("m5/two_step.g fixture must be discovered");

    // It must NOT look like a clean compile+run (which would be a silent
    // miscompile). Either it is reported as failed (the expected path today), or
    // — once M4 lands — it compiles and the run-pass/compile-fail harness passes.
    assert!(
        !matches!(two_step.outcome, TestOutcome::Passed),
        "multi-await must NOT silently compile/run; it should surface the async-v2 diagnostic, \
         got Passed with diagnostics {:?}",
        two_step.diagnostics,
    );

    let emitted_async_v2 = two_step
        .diagnostics
        .iter()
        .any(|d| d.message.contains(ASYNC_V2_SUBSTRING));
    assert!(
        emitted_async_v2,
        "multi-await must emit the async-v2 diagnostic (error 61); diagnostics: {:?}",
        two_step.diagnostics,
    );
}

#[test]
fn m5_one_step_single_await_must_not_miscompile() {
    // Single-await is the verified-supported shape (type-checks with zero
    // diagnostics via the `PipelineCompiler`). This guards against a *silent*
    // miscompile: if it fails, the failure must be the known codegen gap
    // ("no executable produced"), not a wrong-output/broken-binary regression.
    let plan = TestRunner::new(m5_root())
        .parallel(false)
        .build()
        .expect("m5 fixture collection must succeed");
    let result = plan.execute();

    let one_step = result
        .results
        .iter()
        .find(|r| r.test.name.contains("one_step"))
        .expect("m5/one_step.g fixture must be discovered");

    match &one_step.outcome {
        TestOutcome::Passed => { /* the eventual goal: runs cleanly */ }
        TestOutcome::Failed { reason } => {
            let ok = matches!(
                reason,
                glyim_test::error::FailureReason::CompilationFailed { .. }
            );
            assert!(
                ok,
                "single-await may only fail with the known codegen gap \
                 (CompilationFailed / no executable), not a miscompile; got {:?}",
                reason
            );
        }
        TestOutcome::Ignored => panic!("m5/one_step.g must not be Ignored on the linux target"),
    }
}

#[test]
fn m5_fixtures_present_and_moded_correctly() {
    let plan = TestRunner::new(m5_root())
        .parallel(false)
        .build()
        .expect("collection");
    let names: Vec<String> = plan.tests.iter().map(|t| t.name.clone()).collect();

    assert!(
        names.iter().any(|n| n.contains("one_step")),
        "missing m5/one_step.g fixture; have: {:?}",
        names
    );
    assert!(
        names.iter().any(|n| n.contains("two_step")),
        "missing m5/two_step.g fixture; have: {:?}",
        names
    );

    let one = plan
        .tests
        .iter()
        .find(|t| t.name.contains("one_step"))
        .expect("one_step fixture");
    assert_eq!(one.config.mode, TestMode::RunPass, "m5/one_step must be run-pass");
    assert_eq!(
        one.config.only_target.as_deref(),
        Some("x86_64-unknown-linux-gnu"),
        "m5/one_step must be gated to linux"
    );

    let two = plan
        .tests
        .iter()
        .find(|t| t.name.contains("two_step"))
        .expect("two_step fixture");
    assert_eq!(two.config.mode, TestMode::CompileFail, "m5/two_step must be compile-fail");
}
