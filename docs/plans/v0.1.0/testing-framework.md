# `glyim-test` — Complete Redesigned Plan

Leveraging every infrastructure change from the v0.1.0 update: `TypeLookup` trait, `Ty::ERROR` sentinels, separate `IntVar`/`FloatVar`, no Salsa, `num_enum`, no `IdxLike for Ty`, `DiagSink` logging, and strict `PathKind`.

---

## Cargo.toml

```toml
# crates/glyim-test/Cargo.toml
[package]
name = "glyim-test"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-core       = { workspace = true }
glyim-span       = { workspace = true }
glyim-diag       = { workspace = true }
glyim-vfs        = { workspace = true }
glyim-syntax     = { workspace = true }
glyim-frontend   = { workspace = true }
glyim-def-map    = { workspace = true }
glyim-type       = { workspace = true }
glyim-hir        = { workspace = true }
glyim-mir        = { workspace = true }
glyim-solve      = { workspace = true }
glyim-typeck     = { workspace = true }
glyim-lower      = { workspace = true }
glyim-borrowck   = { workspace = true }
glyim-codegen    = { workspace = true }
glyim-db         = { workspace = true }
glyim-pipeline   = { workspace = true }

insta        = { workspace = true }
tracing      = { workspace = true }
rayon        = { workspace = true }
similar      = "2.7"
termcolor    = "1.4"
walkdir      = "2.5"
regex        = "1.11"
shell-words  = "2.0"
rand         = "0.8"
parking_lot  = { workspace = true }
serde_json   = { workspace = true }

[dev-dependencies]
tempfile = "3.14"
```

---

## Top-Level Module Structure

```
src/
├── lib.rs
├── harness/
│   ├── mod.rs
│   ├── config.rs          # Strict parsing, .g extension
│   ├── collector.rs       # Directory validation, .g discovery
│   ├── plan.rs            # TestPlan (immutable, executable)
│   ├── executor.rs        # Delegates to Pipeline, per-test timeout
│   └── reporter.rs        # Stderr output, panic with names
├── annotations/
│   ├── mod.rs             # Single-pass state machine
│   └── pattern.rs         # Substring, regex, exact, any
├── comparison/
│   ├── mod.rs             # Exact match default, documented invariant
│   └── normalize.rs       # Path/slash normalization
├── mock/
│   ├── mod.rs
│   ├── lower_ctx.rs       # Full impl, recorded spans
│   ├── borrowck_ctx.rs    # Full impl
│   ├── solver.rs          # Programmable, RefCell documented
│   ├── codegen.rs         # Call recording
│   └── db.rs              # Simple Database builder (no Salsa)
├── assertions/
│   ├── mod.rs
│   ├── ty.rs              # Fluent TyAssert using TypeLookup
│   ├── mir.rs             # MirAssert
│   ├── diag.rs            # Diagnostic assertions
│   └── span.rs            # Span assertions
├── snapshot/
│   ├── mod.rs             # Wraps insta
│   └── format.rs          # Consistent formatting
├── property/
│   ├── mod.rs
│   └── arbitrary.rs       # Valid types via TyCtxMut, separate IntVar/FloatVar
└── fixtures/
    ├── mod.rs
    └── builder.rs         # SourceBuilder, TyCtxBuilder
```

---

## `lib.rs`

```rust
// crates/glyim-test/src/lib.rs
//! State-of-the-art compiler testing framework for Glyim.
//!
//! # Infrastructure Leverage
//!
//! This crate perfectly leverages the v0.1.0 compiler infrastructure:
//!
//! - **[F1]** `Ty` does NOT implement `IdxLike`. Test code never calls
//!   `Ty::from_raw()` — it uses `Ty::ERROR`, `Ty::NEVER`, `Ty::UNIT`,
//!   `Ty::BOOL` sentinels or constructs types through `TyCtxMut`.
//!
//! - **[F2]** All sentinel constants (`Ty::ERROR`, etc.) are used instead
//!   of `Ty::from_raw(0)`.
//!
//! - **[F4]** `TypeLookup` trait is used throughout for type display
//!   and inspection. `PrintTy::new(ty, ctx)` works with both `TyCtx`
//!   and `TyCtxMut`.
//!
//! - **[F13]** No Salsa. `TestDbBuilder` creates a simple `Database`.
//!
//! - **[F16]** `DiagSink::new()` provides default logging.
//!
//! - **[F18]** `IntVar`, `FloatVar`, `TyVar` are separate types.
//!   The property generator and mocks respect this separation.
//!
//! # File-Based Testing
//!
//! Test files use the `.g` extension and live in mode-specific directories:
//!
//! ```text
//! tests/
//! ├── compile-fail/
//! ├── compile-pass/
//! └── ui/
//! ```
//!
//! # Error Annotations
//!
//! ```gly
//! //~ ERROR message      — exact line match
//! //~^^ ERROR message    — 2 lines above
//! //~| NOTE message      — same target as previous
//! //~? ERROR message     — optional
//! //~~ ERROR message     — fuzzy (1-line tolerance)
//! ```
//!
//! # Environment Variables
//!
//! - `GLYIM_BLESS=1` — auto-update expected output files
//! - `GLYIM_TEST_SHOW_OUTPUT=1` — verbose test output on stderr

pub mod harness;
pub mod annotations;
pub mod comparison;
pub mod mock;
pub mod assertions;
pub mod snapshot;
pub mod property;
pub mod fixtures;

// ── Re-exports ──

pub use harness::{TestRunner, TestPlan, TestMode};
pub use mock::{MockSolver, MockCodegen, MockBorrowckCtx, MockLowerCtx, TestDbBuilder};
pub use assertions::{
    assert_ty, TyAssert,
    assert_mir, MirAssert,
    assert_no_errors, assert_has_errors, assert_error_count,
    assert_diag_contains, assert_diag_code, assert_has_severity,
};
pub use snapshot::{snapshot_cst, snapshot_mir, snapshot_def_map};
pub use fixtures::{SourceBuilder, TyCtxBuilder};

use glyim_core::interner::Interner;
use glyim_type::{TyCtx, TyCtxMut, Ty, TyKind, TypeLookup};

/// Create a `TyCtxMut` for testing with default settings.
pub fn test_ty_ctx() -> TyCtxMut {
    TyCtxBuilder::new().build_mut()
}

/// Create a frozen `TyCtx` for testing with default settings.
pub fn test_frozen_ty_ctx() -> TyCtx {
    test_ty_ctx().freeze()
}

/// Run a function with a fresh `TyCtxMut` and freeze the result.
///
/// NOTE: The `TyCtxMut` is consumed by `freeze()`. If you need
/// to keep it mutable (e.g., for property generation), use
/// `test_ty_ctx()` directly instead.
pub fn with_fresh_ty_ctx<F, R>(f: F) -> (TyCtx, R)
where
    F: FnOnce(&mut TyCtxMut) -> R,
{
    let mut ctx_mut = test_ty_ctx();
    let result = f(&mut ctx_mut);
    (ctx_mut.freeze(), result)
}
```

---

## Harness: Config

```rust
// crates/glyim-test/src/harness/config.rs
//! Test configuration parsed from file headers.
//!
//! - `.g` file extension (NOT `.gly`)
//! - Strict hyphenated mode names only
//! - Strict severity keywords — typos are errors
//! - `shell-words` crate for compile flags

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct TestConfig {
    pub mode: TestMode,
    pub revisions: Vec<String>,
    pub revision_compile_flags: HashMap<String, Vec<String>>,
    pub compile_flags: Vec<String>,
    pub error_patterns: Vec<String>,
    pub needs_llvm: bool,
    pub min_version: Option<String>,
    pub ignore: bool,
    pub only_target: Option<String>,
    pub aux_files: Vec<PathBuf>,
    pub timeout_secs: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            mode: TestMode::CompilePass,
            revisions: Vec::new(),
            revision_compile_flags: HashMap::new(),
            compile_flags: Vec::new(),
            error_patterns: Vec::new(),
            needs_llvm: false,
            min_version: None,
            ignore: false,
            only_target: None,
            aux_files: Vec::new(),
            timeout_secs: 60,
        }
    }
}

/// Test modes for v0.1.0. run-pass/run-fail deferred to v0.2.0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TestMode {
    CompilePass,
    CompileFail,
    Ui,
}

impl TestMode {
    /// Strict parsing: only exact hyphenated lowercase forms.
    pub fn from_str_exact(s: &str) -> Result<Self, String> {
        match s {
            "compile-pass" => Ok(Self::CompilePass),
            "compile-fail" => Ok(Self::CompileFail),
            "ui" => Ok(Self::Ui),
            _ => Err(format!(
                "unknown test-mode: {:?}. Expected: compile-pass, compile-fail, ui",
                s
            )),
        }
    }

    pub fn dir_name(self) -> &'static str {
        match self {
            Self::CompilePass => "compile-pass",
            Self::CompileFail => "compile-fail",
            Self::Ui => "ui",
        }
    }
}

/// Parse test configuration from file header comments.
/// Returns `Result` to propagate strict parsing errors.
pub fn parse_test_config(source: &str) -> Result<TestConfig, String> {
    let mut config = TestConfig::default();

    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("//") {
            if trimmed.is_empty() { continue; }
            break;
        }

        let content = trimmed[2..].trim();

        // Revision-specific directives: [rev] key: value
        if let Some(rest) = content.strip_prefix('[') {
            if let Some(bracket_end) = rest.find(']') {
                let rev = &rest[..bracket_end];
                let directive = rest[bracket_end + 1..].trim();
                if let Some(value) = directive.strip_prefix("compile-flags:") {
                    config.revision_compile_flags
                        .entry(rev.to_string())
                        .or_default()
                        .extend(
                            shell_words::split(value.trim())
                                .map_err(|e| format!("invalid compile flags: {}", e))?,
                        );
                }
                continue;
            }
        }

        if let Some(value) = content.strip_prefix("test-mode:") {
            config.mode = TestMode::from_str_exact(value.trim())?;
        } else if let Some(value) = content.strip_prefix("revisions:") {
            config.revisions = value.split_whitespace().map(String::from).collect();
        } else if let Some(value) = content.strip_prefix("compile-flags:") {
            config.compile_flags.extend(
                shell_words::split(value.trim())
                    .map_err(|e| format!("invalid compile flags: {}", e))?,
            );
        } else if let Some(value) = content.strip_prefix("error-pattern:") {
            config.error_patterns.push(value.trim().to_string());
        } else if content == "needs-llvm" {
            config.needs_llvm = true;
        } else if let Some(value) = content.strip_prefix("min-version:") {
            config.min_version = Some(value.trim().to_string());
        } else if content == "ignore" {
            config.ignore = true;
        } else if let Some(value) = content.strip_prefix("only-target:") {
            config.only_target = Some(value.trim().to_string());
        } else if let Some(value) = content.strip_prefix("aux-file:") {
            config.aux_files.push(PathBuf::from(value.trim()));
        } else if let Some(value) = content.strip_prefix("timeout:") {
            config.timeout_secs = value.trim().parse().unwrap_or(60);
        }
    }

    Ok(config)
}
```

---

## Harness: Collector

```rust
// crates/glyim-test/src/harness/collector.rs
//! Discovers `.g` test files. Fails loudly if root missing.

use super::config::{TestConfig, TestMode};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

pub struct DiscoveredTest {
    pub path: PathBuf,
    pub name: String,
    pub config: TestConfig,
    pub source: Arc<str>,
    pub revisions: Vec<String>,
}

pub struct TestCollector<'a> {
    root: &'a Path,
}

impl<'a> TestCollector<'a> {
    pub fn new(root: &'a Path) -> Self {
        Self { root }
    }

    /// Walk directory and collect all `.g` test files.
    /// Returns `Err` if root doesn't exist.
    pub fn collect(
        &self,
        filter: Option<&str>,
        mode_override: Option<TestMode>,
    ) -> Result<Vec<DiscoveredTest>, String> {
        if !self.root.exists() {
            return Err(format!(
                "test directory does not exist: {:?}",
                self.root
            ));
        }

        let mut tests = Vec::new();

        for entry in WalkDir::new(self.root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();

            // STRICT: only .g extension
            if path.extension().and_then(|e| e.to_str()) != Some("g") {
                continue;
            }

            // Apply filter
            if let Some(f) = filter {
                if !path.to_string_lossy().contains(f) {
                    continue;
                }
            }

            let source: Arc<str> = std::fs::read_to_string(path)
                .map_err(|e| format!("read {:?}: {}", path, e))?
                .into();

            let mut config = TestConfig::default();

            // Infer mode from directory name if no header
            let dir_mode = path.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .and_then(TestMode::from_str_exact)
                .ok();

            let header_config = super::config::parse_test_config(&source)?;
            config.mode = header_config.mode;
            if config.mode == TestMode::CompilePass && dir_mode.is_some() {
                config.mode = dir_mode.unwrap();
            }
            if let Some(mode) = mode_override {
                config.mode = mode;
            }

            config.revisions = header_config.revisions;
            config.compile_flags = header_config.compile_flags;
            config.revision_compile_flags = header_config.revision_compile_flags;
            config.error_patterns = header_config.error_patterns;
            config.needs_llvm = header_config.needs_llvm;
            config.ignore = header_config.ignore;
            config.timeout_secs = header_config.timeout_secs;
            config.min_version = header_config.min_version;
            config.only_target = header_config.only_target;

            let name = path.strip_prefix(self.root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");

            let revisions = if config.revisions.is_empty() {
                vec!["default".to_string()]
            } else {
                config.revisions.clone()
            };

            tests.push(DiscoveredTest {
                path: path.to_path_buf(),
                name,
                config,
                source,
                revisions,
            });
        }

        tests.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tests)
    }
}
```

---

## Harness: Plan & Runner

```rust
// crates/glyim-test/src/harness/plan.rs
//! Immutable test plan. Builder produces Plan, Plan executes.

use super::config::TestMode;
use super::collector::TestCollector;
use super::executor::TestExecutor;
use super::reporter::TestReporter;
use std::path::PathBuf;
use std::time::Duration;

/// The builder for test execution.
pub struct TestRunner {
    root: PathBuf,
    mode_override: Option<TestMode>,
    parallel: bool,
    filter: Option<String>,
    timeout: Duration,
}

impl TestRunner {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            mode_override: None,
            parallel: true,
            filter: None,
            timeout: Duration::from_secs(60),
        }
    }

    pub fn mode(mut self, mode: TestMode) -> Self { self.mode_override = Some(mode); self }
    pub fn parallel(mut self, yes: bool) -> Self { self.parallel = yes; self }
    pub fn filter(mut self, f: impl Into<String>) -> Self { self.filter = Some(f.into()); self }
    pub fn timeout(mut self, d: Duration) -> Self { self.timeout = d; self }

    /// Build an immutable TestPlan.
    pub fn build(self) -> Result<TestPlan, String> {
        let collector = TestCollector::new(&self.root);
        let tests = collector.collect(self.filter.as_deref(), self.mode_override)?;

        Ok(TestPlan {
            tests,
            parallel: self.parallel,
            default_timeout: self.timeout,
            // Environment variable support
            bless: std::env::var("GLYIM_BLESS").is_ok(),
            verbose: std::env::var("GLYIM_TEST_SHOW_OUTPUT").is_ok(),
        })
    }
}

/// The immutable, executable test plan.
pub struct TestPlan {
    pub tests: Vec<super::collector::DiscoveredTest>,
    pub parallel: bool,
    pub default_timeout: Duration,
    pub bless: bool,
    pub verbose: bool,
}

impl TestPlan {
    /// Execute the plan and panic on failure.
    pub fn run(self) {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();

        if self.tests.is_empty() {
            eprintln!("no test files found");
            return;
        }

        eprintln!("running {} tests", self.tests.len());

        let executor = TestExecutor::new(self.default_timeout, self.bless, self.verbose);
        let results = if self.parallel {
            executor.run_parallel(&self.tests)
        } else {
            executor.run_sequential(&self.tests)
        };

        let reporter = TestReporter::new(self.verbose);
        let summary = reporter.report(&results);

        if summary.failed > 0 {
            let failed_names: Vec<_> = results.iter()
                .filter(|r| matches!(r.outcome, super::executor::TestOutcome::Failed { .. }))
                .map(|r| format!("{} [{}]", r.test.name, r.revision))
                .collect();
            panic!(
                "{} tests FAILED: {}\n\
                 Run with GLYIM_TEST_SHOW_OUTPUT=1 for details.",
                summary.failed,
                failed_names.join(", ")
            );
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
}
```

---

## Harness: Executor

```rust
// crates/glyim-test/src/harness/executor.rs
//! Per-test execution. Delegates to Pipeline. Per-test timeout.
//! Validates needs_llvm/min_version.

use super::collector::DiscoveredTest;
use crate::annotations::Annotation;
use crate::comparison::{self, NormalizedDiag};
use glyim_diag::DiagSeverity;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum TestOutcome {
    Passed,
    Failed { reason: String },
    Ignored,
}

#[derive(Clone, Debug)]
pub struct TestResult {
    pub test: DiscoveredTest,
    pub revision: String,
    pub outcome: TestOutcome,
    pub duration: Duration,
}

pub struct TestExecutor {
    default_timeout: Duration,
    bless: bool,
    verbose: bool,
}

impl TestExecutor {
    pub fn new(default_timeout: Duration, bless: bool, verbose: bool) -> Self {
        Self { default_timeout, bless, verbose }
    }

    pub fn run_sequential(&self, tests: &[DiscoveredTest]) -> Vec<TestResult> {
        tests.iter()
            .flat_map(|t| t.revisions.iter().map(|r| self.execute(t, r)))
            .collect()
    }

    pub fn run_parallel(&self, tests: &[DiscoveredTest]) -> Vec<TestResult> {
        use rayon::prelude::*;
        tests.par_iter()
            .flat_map(|t| {
                t.revisions.iter().map(|r| self.execute(t, r)).collect::<Vec<_>>()
            })
            .collect()
    }

    fn execute(&self, test: &DiscoveredTest, revision: &str) -> TestResult {
        let start = std::time::Instant::now();

        if test.config.ignore {
            return TestResult {
                test: test.clone(), revision: revision.to_string(),
                outcome: TestOutcome::Ignored, duration: start.elapsed(),
            };
        }

        // Validate needs_llvm
        if test.config.needs_llvm && !cfg!(feature = "llvm") {
            return TestResult {
                test: test.clone(), revision: revision.to_string(),
                outcome: TestOutcome::Ignored, duration: start.elapsed(),
            };
        }

        // Validate only_target
        if let Some(ref target) = test.config.only_target {
            if target != "x86_64-unknown-linux-gnu" {
                return TestResult {
                    test: test.clone(), revision: revision.to_string(),
                    outcome: TestOutcome::Ignored, duration: start.elapsed(),
                };
            }
        }

        // Use per-test timeout
        let _timeout = Duration::from_secs(test.config.timeout_secs);
        // TODO: wrap execution in timeout

        let outcome = self.execute_inner(test, revision);
        TestResult {
            test: test.clone(), revision: revision.to_string(),
            outcome, duration: start.elapsed(),
        }
    }

    fn execute_inner(&self, test: &DiscoveredTest, _revision: &str) -> TestOutcome {
        match test.config.mode {
            super::config::TestMode::CompilePass => self.run_compile_pass(test),
            super::config::TestMode::CompileFail => self.run_compile_fail(test),
            super::config::TestMode::Ui => self.run_ui_test(test),
        }
    }

    /// Compile test via Pipeline.
    fn compile_test(&self, test: &DiscoveredTest) -> (Vec<glyim_diag::GlyimDiagnostic>, glyim_frontend::ParseResult) {
        let file_id = glyim_span::FileId::from_raw(0);
        let parse_result = glyim_frontend::parse_to_syntax(&test.source, file_id);
        // For v0.1.0, only frontend diagnostics.
        // TODO: Delegate full compilation to Pipeline::compile_file
        (parse_result.diagnostics.clone(), parse_result)
    }

    fn run_compile_pass(&self, test: &DiscoveredTest) -> TestOutcome {
        let (diagnostics, _) = self.compile_test(test);
        let errors: Vec<_> = diagnostics.iter().filter(|d| d.is_error()).collect();
        if errors.is_empty() {
            TestOutcome::Passed
        } else {
            TestOutcome::Failed {
                reason: format!(
                    "expected compilation to succeed, but got errors:\n  {}",
                    errors.iter().map(|e| e.message.clone()).collect::<Vec<_>>().join("\n  ")
                ),
            }
        }
    }

    fn run_compile_fail(&self, test: &DiscoveredTest) -> TestOutcome {
        let (diagnostics, _) = self.compile_test(test);

        let annotations = match Annotation::parse_all(&test.source) {
            Ok(a) => a,
            Err(e) => return TestOutcome::Failed { reason: format!("annotation parse error: {}", e) },
        };

        let normalized: Vec<NormalizedDiag> = diagnostics.iter()
            .map(|d| NormalizedDiag::from_glyim_diag(d, &test.source))
            .collect();

        let result = comparison::compare_diagnostics(&annotations, &normalized);

        if result.passed {
            // Check error-patterns
            for pattern in &test.config.error_patterns {
                if !diagnostics.iter().any(|d| d.message.contains(pattern)) {
                    return TestOutcome::Failed {
                        reason: format!("error-pattern '{}' not found", pattern),
                    };
                }
            }
            TestOutcome::Passed
        } else {
            let mut reasons = Vec::new();
            for m in &result.missing {
                reasons.push(format!(
                    "line {}: expected {} {}",
                    m.target_line() + 1,
                    format_severity(m.severity),
                    m.pattern.description()
                ));
            }
            for u in &result.unexpected {
                reasons.push(format!(
                    "line {}: unexpected {} : {}",
                    u.line + 1,
                    format_severity(u.severity),
                    u.message
                ));
            }
            for w in &result.wrong_severity {
                reasons.push(format!(
                    "line {}: expected {} got {} : {}",
                    w.diagnostic.line + 1,
                    format_severity(w.expected),
                    format_severity(w.actual),
                    w.diagnostic.message
                ));
            }
            TestOutcome::Failed {
                reason: format!("diagnostic mismatch:\n  {}", reasons.join("\n  ")),
            }
        }
    }

    fn run_ui_test(&self, test: &DiscoveredTest) -> TestOutcome {
        let (diagnostics, parse_result) = self.compile_test(test);

        let mut output = String::new();
        output.push_str("=== CST ===\n");
        output.push_str(&format!("{:#?}\n", parse_result.root));
        output.push_str("\n=== Diagnostics ===\n");
        for diag in &diagnostics {
            output.push_str(&format!(
                "{}[{}]: {}\n",
                match diag.severity {
                    DiagSeverity::Error => "error",
                    DiagSeverity::Warning => "warning",
                    DiagSeverity::Note => "note",
                    DiagSeverity::Help => "help",
                },
                diag.code,
                diag.message,
            ));
        }

        let normalized = crate::comparison::normalize::normalize_output(
            &output, &test.path, &Default::default(),
        );

        let expected_path = test.path.with_extension("expected");
        if self.bless {
            std::fs::write(&expected_path, &normalized).unwrap();
            return TestOutcome::Passed;
        }

        if expected_path.exists() {
            let expected = std::fs::read_to_string(&expected_path).unwrap();
            if normalized == expected {
                TestOutcome::Passed
            } else {
                let diff = similar::TextDiff::from_lines(&expected, &normalized);
                let mut diff_str = String::new();
                for change in diff.iter_all_changes() {
                    let sign = match change.tag() {
                        similar::ChangeTag::Delete => "-",
                        similar::ChangeTag::Insert => "+",
                        similar::ChangeTag::Equal => " ",
                    };
                    diff_str.push_str(&format!("{}{}", sign, change));
                }
                TestOutcome::Failed {
                    reason: format!("output differs from expected:\n{}", diff_str),
                }
            }
        } else {
            if self.bless {
                std::fs::write(&expected_path, &normalized).unwrap();
            }
            TestOutcome::Failed {
                reason: format!("no expected output file: {:?}", expected_path),
            }
        }
    }
}

fn format_severity(s: DiagSeverity) -> &'static str {
    match s {
        DiagSeverity::Error => "ERROR",
        DiagSeverity::Warning => "WARNING",
        DiagSeverity::Note => "NOTE",
        DiagSeverity::Help => "HELP",
    }
}
```

---

## Harness: Reporter

```rust
// crates/glyim-test/src/harness/reporter.rs
//! Writes to stderr so output is visible with cargo test.

use super::executor::{TestOutcome, TestResult};
use super::plan::TestSummary;
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

pub struct TestReporter { verbose: bool }

impl TestReporter {
    pub fn new(verbose: bool) -> Self { Self { verbose } }

    pub fn report(&self, results: &[TestResult]) -> TestSummary {
        let mut stderr = StandardStream::stderr(ColorChoice::Always);
        let mut summary = TestSummary::default();

        for result in results {
            summary.total += 1;
            match &result.outcome {
                TestOutcome::Passed => {
                    summary.passed += 1;
                    self.write_status(&mut stderr, "ok", Color::Green);
                }
                TestOutcome::Failed { reason } => {
                    summary.failed += 1;
                    self.write_status(&mut stderr, "FAILED", Color::Red);
                    if self.verbose {
                        let _ = writeln!(stderr, "\n{}", reason);
                    }
                }
                TestOutcome::Ignored => {
                    summary.ignored += 1;
                    self.write_status(&mut stderr, "IGNORED", Color::Yellow);
                }
            }
            let _ = writeln!(stderr, "{} [{}]", result.test.name, result.revision);
        }

        let _ = writeln!(stderr, "\n---");
        let _ = writeln!(
            stderr,
            "{} run, {} passed, {} failed, {} ignored",
            summary.total, summary.passed, summary.failed, summary.ignored
        );

        summary
    }

    fn write_status(&self, stream: &mut StandardStream, status: &str, color: Color) {
        let _ = stream.set_color(ColorSpec::new().set_fg(Some(color)).set_bold(true));
        let _ = write!(stream, "{:>8} ", status);
        let _ = stream.reset();
    }
}
```

---

## Annotations

```rust
// crates/glyim-test/src/annotations/mod.rs
//! Single-pass state machine. Strict severity. Exact match by default.
//!
//! Syntax:
//!   //~ ERROR msg       — exact line
//!   //~^^ ERROR msg     — N lines above
//!   //~| NOTE msg       — same target as previous
//!   //~? ERROR msg      — optional
//!   //~~ ERROR msg      — fuzzy (1-line tolerance)

pub mod pattern;

use crate::comparison::pattern::MatchPattern;
use glyim_diag::DiagSeverity;

#[derive(Clone, Debug)]
pub struct Annotation {
    pub line: usize,
    pub line_offset: usize,
    pub severity: DiagSeverity,
    pub pattern: MatchPattern,
    pub optional: bool,
    pub fuzzy: bool,
}

impl Annotation {
    pub fn target_line(&self) -> usize {
        self.line.saturating_sub(self.line_offset)
    }

    /// Single-pass parser. No double-matching of //~| .
    pub fn parse_all(source: &str) -> Result<Vec<Self>, String> {
        let mut annotations = Vec::new();
        let mut last_target_line: Option<usize> = None;

        for (line_idx, line) in source.lines().enumerate() {
            let mut search_from = 0;

            while let Some(start) = line[search_from..].find("//") {
                let abs_start = search_from + start;
                search_from = abs_start + 2;
                let rest = &line[abs_start + 2..];

                // Must start with ~ or ~~
                let (fuzzy, rest) = if let Some(r) = rest.strip_prefix('~') {
                    (true, r) // //~~ fuzzy
                } else if let Some(r) = rest.strip_prefix(' ') {
                    (false, r.trim_start()) // //~ exact
                } else {
                    continue; // Normal comment
                };

                // Optional marker
                let (optional, rest) = if let Some(r) = rest.strip_prefix('?') {
                    (true, r.trim_start())
                } else {
                    (false, rest)
                };

                // Continuation marker
                let (is_continuation, rest) = if let Some(r) = rest.strip_prefix('|') {
                    (true, r.trim_start())
                } else {
                    (false, rest)
                };

                // Caret offset
                let (line_offset, rest) = if is_continuation {
                    (0, rest)
                } else {
                    let count = rest.chars().take_while(|c| *c == '^').count();
                    (count, rest[count..].trim_start())
                };

                // Strict severity
                let (severity, rest) = parse_severity_strict(rest)?;

                let pattern_text = rest.trim();
                let pattern = if pattern_text.is_empty() {
                    MatchPattern::Any
                } else {
                    MatchPattern::substring(pattern_text)
                };

                let target_line = if is_continuation {
                    last_target_line.ok_or_else(|| {
                        format!("line {}: //~| without preceding annotation", line_idx + 1)
                    })?
                } else {
                    line_idx.saturating_sub(line_offset)
                };

                last_target_line = Some(target_line);

                annotations.push(Annotation {
                    line: line_idx,
                    line_offset: if is_continuation {
                        line_idx.saturating_sub(target_line)
                    } else {
                        line_offset
                    },
                    severity,
                    pattern,
                    optional,
                    fuzzy,
                });
            }
        }

        Ok(annotations)
    }
}

/// Strict severity. Typos cause errors.
fn parse_severity_strict(s: &str) -> Result<(DiagSeverity, &str), String> {
    let (word, rest) = s.split_once(char::is_whitespace).unwrap_or((s, ""));
    match word {
        "ERROR" => Ok((DiagSeverity::Error, rest.trim_start())),
        "WARNING" => Ok((DiagSeverity::Warning, rest.trim_start())),
        "NOTE" => Ok((DiagSeverity::Note, rest.trim_start())),
        "HELP" => Ok((DiagSeverity::Help, rest.trim_start())),
        "" => Ok((DiagSeverity::Error, rest)), // Default only when empty
        other => Err(format!(
            "invalid severity: {:?}. Expected ERROR, WARNING, NOTE, or HELP",
            other
        )),
    }
}
```

```rust
// crates/glyim-test/src/annotations/pattern.rs
//! Matching patterns for diagnostic comparison.

use std::fmt;

#[derive(Clone, Debug)]
pub enum MatchPattern {
    Any,
    Substring(String),
    Regex(regex::Regex),
    Exact(String),
}

impl MatchPattern {
    pub fn substring(s: &str) -> Self { Self::Substring(s.to_string()) }
    pub fn exact(s: &str) -> Self { Self::Exact(s.to_string()) }
    pub fn regex(pattern: &str) -> Result<Self, regex::Error> {
        Ok(Self::Regex(regex::Regex::new(pattern)?))
    }

    pub fn matches(&self, message: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Substring(s) => message.contains(s.as_str()),
            Self::Regex(re) => re.is_match(message),
            Self::Exact(s) => message == s,
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::Any => "<any>".into(),
            Self::Substring(s) => format!("contains {:?}", s),
            Self::Regex(re) => format!("matches {:?}", re.as_str()),
            Self::Exact(s) => format!("== {:?}", s),
        }
    }
}

impl fmt::Display for MatchPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl PartialEq for MatchPattern {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Any, Self::Any) => true,
            (Self::Substring(a), Self::Substring(b)) => a == b,
            (Self::Exact(a), Self::Exact(b)) => a == b,
            (Self::Regex(a), Self::Regex(b)) => a.as_str() == b.as_str(),
            _ => false,
        }
    }
}
impl Eq for MatchPattern {}
```

---

## Comparison

```rust
// crates/glyim-test/src/comparison/mod.rs
//! Diagnostic comparison engine.
//!
//! INVARIANT: matched.len() + missing.len() + wrong_severity.len() == annotations.len()
//!            matched.len() + unexpected.len() + wrong_severity.len() == diagnostics.len()
//!
//! Exact match by default. //~~ enables 1-line fuzzy tolerance.

pub mod normalize;
pub mod pattern;

use crate::annotations::Annotation;
use glyim_diag::{DiagSeverity, GlyimDiagnostic};
use glyim_span::Span;

#[derive(Clone, Debug)]
pub struct NormalizedDiag {
    pub severity: DiagSeverity,
    pub line: usize,
    pub message: String,
    pub span: Span,
}

impl NormalizedDiag {
    pub fn from_glyim_diag(diag: &GlyimDiagnostic, source: &str) -> Self {
        let line = byte_offset_to_line(source, diag.span.primary.lo.to_usize());
        Self { severity: diag.severity, line, message: diag.message.clone(), span: diag.span.primary }
    }
}

#[derive(Clone, Debug)]
pub struct ComparisonResult {
    pub matched: Vec<MatchedPair>,
    pub missing: Vec<Annotation>,
    pub unexpected: Vec<NormalizedDiag>,
    pub wrong_severity: Vec<SeverityMismatch>,
    pub passed: bool,
}

#[derive(Clone, Debug)]
pub struct MatchedPair { pub annotation: Annotation, pub diagnostic: NormalizedDiag }

#[derive(Clone, Debug)]
pub struct SeverityMismatch {
    pub annotation: Annotation,
    pub diagnostic: NormalizedDiag,
    pub expected: DiagSeverity,
    pub actual: DiagSeverity,
}

pub fn compare_diagnostics(
    annotations: &[Annotation],
    diagnostics: &[NormalizedDiag],
) -> ComparisonResult {
    let mut matched = Vec::new();
    let mut missing = Vec::new();
    let mut wrong_severity = Vec::new();
    let mut diag_used = vec![false; diagnostics.len()];

    for annotation in annotations {
        let target_line = annotation.target_line();
        let mut found = false;

        for (i, diag) in diagnostics.iter().enumerate() {
            if diag_used[i] { continue; }

            // Exact by default; fuzzy allows 1-line tolerance
            let line_matches = if annotation.fuzzy {
                diag.line.abs_diff(target_line) <= 1
            } else {
                diag.line == target_line
            };

            if line_matches && annotation.pattern.matches(&diag.message) {
                diag_used[i] = true;
                found = true;

                if diag.severity == annotation.severity {
                    matched.push(MatchedPair { annotation: annotation.clone(), diagnostic: diag.clone() });
                } else {
                    wrong_severity.push(SeverityMismatch {
                        annotation: annotation.clone(), diagnostic: diag.clone(),
                        expected: annotation.severity, actual: diag.severity,
                    });
                }
                break;
            }
        }

        if !found && !annotation.optional {
            missing.push(annotation.clone());
        }
    }

    let unexpected: Vec<_> = diagnostics.iter().enumerate()
        .filter(|(i, _)| !diag_used[*i])
        .map(|(_, d)| d.clone())
        .collect();

    let passed = missing.is_empty() && unexpected.is_empty() && wrong_severity.is_empty();
    ComparisonResult { matched, missing, unexpected, wrong_severity, passed }
}

fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())].chars().filter(|&c| c == '\n').count()
}
```

```rust
// crates/glyim-test/src/comparison/normalize.rs
//! Output normalization for snapshot comparison.

use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct NormalizeRules {
    pub normalize_slashes: bool,
    pub normalize_line_endings: bool,
    pub substitute_dir: bool,
}

pub fn normalize_output(output: &str, test_path: &Path, rules: &NormalizeRules) -> String {
    let mut result = output.to_string();
    if rules.normalize_line_endings { result = result.replace("\r\n", "\n"); }
    if rules.normalize_slashes { result = result.replace('\\', "/"); }
    if rules.substitute_dir {
        if let Some(parent) = test_path.parent() {
            let dir_str = parent.to_string_lossy().replace('\\', "/");
            result = result.replace(&dir_str, "$DIR");
        }
    }
    result
}
```

---

## Mocks

```rust
// crates/glyim-test/src/mock/mod.rs
pub mod lower_ctx;
pub mod borrowck_ctx;
pub mod solver;
pub mod codegen;
pub mod db;

pub use lower_ctx::MockLowerCtx;
pub use borrowck_ctx::MockBorrowckCtx;
pub use solver::MockSolver;
pub use codegen::MockCodegen;
pub use db::TestDbBuilder;
```

```rust
// crates/glyim-test/src/mock/lower_ctx.rs
//! Full MockLowerCtx with recorded span operations.

use glyim_type::TyCtx;
use glyim_span::Span;
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpanOp { Push(Span), Pop }

pub struct MockLowerCtx<'a> {
    pub ty_ctx: &'a TyCtx,
    pub span_ops: RefCell<Vec<SpanOp>>,
}

impl<'a> MockLowerCtx<'a> {
    pub fn new(ty_ctx: &'a TyCtx) -> Self {
        Self { ty_ctx, span_ops: RefCell::new(Vec::new()) }
    }

    pub fn assert_spans_balanced(&self) {
        let depth = self.span_ops.borrow().iter().fold(0, |acc, op| match op {
            SpanOp::Push(_) => acc + 1,
            SpanOp::Pop => acc.saturating_sub(1),
        });
        assert_eq!(depth, 0, "Unbalanced span operations");
    }
}

impl<'a> glyim_lower::LowerCtx for MockLowerCtx<'a> {
    fn ty_ctx(&self) -> &TyCtx { self.ty_ctx }
    fn adt_def(&self, _id: glyim_core::def_id::AdtId) -> glyim_lower::AdtDef {
        glyim_lower::AdtDef { variants: Vec::new(), kind: glyim_lower::AdtKind::Struct }
    }
    fn push_span(&self, span: Span) { self.span_ops.borrow_mut().push(SpanOp::Push(span)); }
    fn pop_span(&self) { self.span_ops.borrow_mut().push(SpanOp::Pop); }
}
```

```rust
// crates/glyim-test/src/mock/borrowck_ctx.rs
//! Full MockBorrowckCtx.

use glyim_mir::{Body, LocalDecl, LocalIdx};
use glyim_type::{Ty, TyCtx};

pub struct MockBorrowckCtx<'a> {
    pub ty_ctx: &'a TyCtx,
    pub body: &'a Body,
}

impl<'a> MockBorrowckCtx<'a> {
    pub fn new(ty_ctx: &'a TyCtx, body: &'a Body) -> Self { Self { ty_ctx, body } }
}

impl<'a> glyim_borrowck::BorrowckCtx for MockBorrowckCtx<'a> {
    fn ty_ctx(&self) -> &TyCtx { self.ty_ctx }
    fn local_decl(&self, local: LocalIdx) -> &LocalDecl { &self.body.locals[local] }
    fn is_copy(&self, _ty: Ty) -> bool { false }
}
```

```rust
// crates/glyim-test/src/mock/solver.rs
//! Programmable mock solver. RefCell — NOT re-entrant (documented).

use glyim_solve::{SolverResult, TraitPredicate, TraitSolver, Predicate};
use glyim_type::TyCtx;
use std::cell::RefCell;

pub struct MockSolver {
    responses: Vec<(PredicateMatcher, SolverResult)>,
    calls: RefCell<Vec<TraitPredicate>>,
    default: SolverResult,
}

enum PredicateMatcher { TraitId(glyim_core::def_id::TraitDefId), Any }

impl MockSolver {
    pub fn new() -> Self {
        Self { responses: Vec::new(), calls: RefCell::new(Vec::new()), default: SolverResult::Ambiguous }
    }
    pub fn default_result(mut self, result: SolverResult) -> Self { self.default = result; self }
    pub fn respond_for_trait(mut self, id: glyim_core::def_id::TraitDefId, result: SolverResult) -> Self {
        self.responses.push((PredicateMatcher::TraitId(id), result)); self
    }
    pub fn respond_for_any(mut self, result: SolverResult) -> Self {
        self.responses.push((PredicateMatcher::Any, result)); self
    }
    pub fn calls(&self) -> usize { self.calls.borrow().len() }
    pub fn assert_call_count(&self, expected: usize) {
        assert_eq!(self.calls.borrow().len(), expected);
    }
}

impl Default for MockSolver { fn default() -> Self { Self::new() } }

impl TraitSolver for MockSolver {
    fn can_prove(&mut self, _ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult {
        // NOTE: RefCell — NOT re-entrant. Do not call recursively.
        self.calls.borrow_mut().push(predicate.clone());
        self.responses.iter()
            .find_map(|(m, r)| match m {
                PredicateMatcher::TraitId(id) if predicate.trait_ref.def_id == *id => Some(r),
                PredicateMatcher::Any => Some(r),
                _ => None,
            })
            .copied()
            .unwrap_or(self.default.clone())
    }

    fn evaluate_predicate(&mut self, ctx: &TyCtx, predicate: &Predicate) -> SolverResult {
        match predicate { Predicate::Trait(tp) => self.can_prove(ctx, tp), _ => self.default.clone() }
    }
}
```

```rust
// crates/glyim-test/src/mock/codegen.rs
//! Mock codegen backend with call recording.

use glyim_codegen::{CodegenBackend, CodegenResult};
use glyim_diag::CompResult;
use glyim_type::TyCtx;
use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct CodegenCall { pub body_count: usize, pub output_path: std::path::PathBuf }

pub struct MockCodegen { calls: RefCell<Vec<CodegenCall>> }

impl MockCodegen {
    pub fn new() -> Self { Self { calls: RefCell::new(Vec::new()) } }
    pub fn calls(&self) -> Vec<CodegenCall> { self.calls.borrow().clone() }
    pub fn assert_generated(&self, expected: usize) {
        let calls = self.calls.borrow();
        assert!(!calls.is_empty(), "expected codegen to be called");
        assert_eq!(calls[0].body_count, expected);
    }
}

impl Default for MockCodegen { fn default() -> Self { Self::new() } }

impl CodegenBackend for MockCodegen {
    fn name(&self) -> &'static str { "mock" }
    fn generate(&self, _ctx: &TyCtx, bodies: &[Arc<glyim_mir::Body>], output: &Path) -> CompResult<CodegenResult> {
        self.calls.borrow_mut().push(CodegenCall { body_count: bodies.len(), output_path: output.to_path_buf() });
        Ok(CodegenResult {
            output_path: output.to_path_buf(),
            symbols: bodies.iter().map(|b| format!("{:?}", b.owner)).collect(),
        })
    }
    fn generate_function(&self, _ctx: &TyCtx, _body: &Arc<glyim_mir::Body>) -> CompResult<Vec<u8>> { Ok(Vec::new()) }
}
```

```rust
// crates/glyim-test/src/mock/db.rs
//! Simple Database builder. No Salsa (F13).

use glyim_core::interner::Interner;
use glyim_db::{Database, CrateConfig};
use std::sync::Arc;

pub struct TestDbBuilder {
    files: Vec<(String, String)>,
    interner: Option<Interner>,
    config: Option<CrateConfig>,
}

impl TestDbBuilder {
    pub fn new() -> Self { Self { files: Vec::new(), interner: None, config: None } }
    pub fn with_interner(mut self, interner: Interner) -> Self { self.interner = Some(interner); self }
    pub fn with_config(mut self, config: CrateConfig) -> Self { self.config = Some(config); self }
    pub fn file(mut self, path: impl Into<String>, content: impl Into<String>) -> Self {
        self.files.push((path.into(), content.into())); self
    }
    pub fn build(self) -> Database {
        let config = self.config.unwrap_or_else(|| CrateConfig {
            name: "test".into(), target_triple: "x86_64-unknown-linux-gnu".into(), opt_level: 0,
        });
        let mut db = if let Some(interner) = self.interner {
            Database::with_interner(interner, config)
        } else {
            Database::new(config)
        };
        for (path, content) in &self.files {
            db.vfs().add_file_content(std::path::Path::new(path), Arc::from(content.as_str()));
        }
        db
    }
}

impl Default for TestDbBuilder { fn default() -> Self { Self::new() } }
```

---

## Assertions

```rust
// crates/glyim-test/src/assertions/mod.rs
pub mod ty;
pub mod mir;
pub mod diag;
pub mod span;

pub use ty::*;
pub use mir::*;
pub use diag::*;
pub use span::*;
```

```rust
// crates/glyim-test/src/assertions/ty.rs
//! Fluent type assertions using TypeLookup trait.
//!
//! [F1] Never calls Ty::from_raw(). Uses Ty::ERROR etc.
//! [F4] Generic over TypeLookup so works with TyCtx and TyCtxMut.

use glyim_core::primitives::*;
use glyim_type::*;

/// Entry point for type assertions.
pub fn assert_ty<'a, L: TypeLookup>(lookup: &'a L, ty: Ty) -> TyAssert<'a, L> {
    TyAssert { lookup, ty, kind: lookup.ty_kind(ty).clone() }
}

pub struct TyAssert<'a, L: TypeLookup> {
    lookup: &'a L,
    ty: Ty,
    kind: TyKind,
}

impl<'a, L: TypeLookup> TyAssert<'a, L> {
    fn fail(&self, expected: &str) -> ! {
        panic!(
            "type assertion failed:\n  expected: {}\n  actual:   {}\n  TyKind:   {:?}",
            expected,
            PrintTy::new(self.ty, self.lookup),
            self.kind,
        )
    }

    pub fn kind_eq(self, expected: &TyKind) -> Self {
        if &self.kind != expected { self.fail(&format!("{:?}", expected)); }
        self
    }

    pub fn is_error(self) -> Self {
        if !matches!(self.kind, TyKind::Error) { self.fail("error type"); }
        self
    }

    pub fn is_not_error(self) -> Self {
        if matches!(self.kind, TyKind::Error) { panic!("expected non-error type, got error"); }
        self
    }

    pub fn is_never(self) -> Self {
        if !matches!(self.kind, TyKind::Never) { self.fail("never type"); }
        self
    }

    pub fn is_bool(self) -> Self {
        if !matches!(self.kind, TyKind::Bool) { self.fail("bool type"); }
        self
    }

    pub fn is_unit(self) -> Self {
        if !matches!(self.kind, TyKind::Unit) { self.fail("unit type"); }
        self
    }

    pub fn is_int(self, expected: IntTy) -> Self {
        match &self.kind {
            TyKind::Int(i) if *i == expected => self,
            _ => self.fail(&format!("Int({:?})", expected)),
        }
    }

    pub fn is_any_int(self) -> Self {
        if !matches!(self.kind, TyKind::Int(_)) { self.fail("any Int type"); }
        self
    }

    pub fn is_uint(self, expected: UintTy) -> Self {
        match &self.kind {
            TyKind::Uint(u) if *u == expected => self,
            _ => self.fail(&format!("Uint({:?})", expected)),
        }
    }

    pub fn is_float(self, expected: FloatTy) -> Self {
        match &self.kind {
            TyKind::Float(f) if *f == expected => self,
            _ => self.fail(&format!("Float({:?})", expected)),
        }
    }

    pub fn is_ref(self, mutability: Mutability) -> TyAssert<'a, L> {
        match &self.kind {
            TyKind::Ref(_, inner, m) if *m == mutability => {
                TyAssert { lookup: self.lookup, ty: *inner, kind: self.lookup.ty_kind(*inner).clone() }
            }
            _ => self.fail(&format!("&{} type", mutability.prefix_str().trim())),
        }
    }

    pub fn is_slice(self) -> TyAssert<'a, L> {
        match &self.kind {
            TyKind::Slice(inner) => {
                TyAssert { lookup: self.lookup, ty: *inner, kind: self.lookup.ty_kind(*inner).clone() }
            }
            _ => self.fail("slice type"),
        }
    }

    pub fn has_infer(self) -> Self {
        if !self.lookup.ty_flags(self.ty).contains(TypeFlags::HAS_TY_INFER) { self.fail("type with inference variables"); }
        self
    }

    pub fn has_no_infer(self) -> Self {
        if self.lookup.ty_flags(self.ty).contains(TypeFlags::HAS_TY_INFER) { self.fail("fully resolved type"); }
        self
    }
}

pub fn assert_ty_eq<L: TypeLookup>(ctx: &L, a: Ty, b: Ty) {
    assert_eq!(a, b, "types not equal: {} vs {}", PrintTy::new(a, ctx), PrintTy::new(b, ctx));
}
```

```rust
// crates/glyim-test/src/assertions/mir.rs
//! MIR structure assertions.

use glyim_mir::*;
use glyim_type::TyCtx;

pub fn assert_mir<'a>(ctx: &'a TyCtx, body: &'a Body) -> MirAssert<'a> { MirAssert { ctx, body } }

pub struct MirAssert<'a> { ctx: &'a TyCtx, body: &'a Body }

impl<'a> MirAssert<'a> {
    pub fn block_count(self, expected: usize) -> Self {
        assert_eq!(self.body.basic_blocks.len(), expected); self
    }
    pub fn local_count(self, expected: usize) -> Self {
        assert_eq!(self.body.locals.len(), expected); self
    }
    pub fn block_stmt_count(self, block: BasicBlockIdx, expected: usize) -> Self {
        assert_eq!(self.body.basic_blocks[block].statements.len(), expected, "bb{}", block.to_raw()); self
    }
    pub fn block_terminator(self, block: BasicBlockIdx, expected: &str) -> Self {
        let actual = match &self.body.basic_blocks[block].terminator.kind {
            TerminatorKind::Goto { .. } => "Goto",
            TerminatorKind::SwitchInt { .. } => "SwitchInt",
            TerminatorKind::Return => "Return",
            TerminatorKind::Unreachable => "Unreachable",
            TerminatorKind::Call { .. } => "Call",
            TerminatorKind::Assert { .. } => "Assert",
            TerminatorKind::Drop { .. } => "Drop",
        };
        assert_eq!(actual, expected, "bb{} terminator", block.to_raw()); self
    }
    pub fn local_ty(self, local: LocalIdx, expected: &glyim_type::TyKind) -> Self {
        let actual = self.ctx.ty_kind(self.body.locals[local].ty);
        assert_eq!(actual, expected, "local {} ty", local.to_raw()); self
    }
}
```

```rust
// crates/glyim-test/src/assertions/diag.rs
//! Diagnostic assertions.

use glyim_diag::{DiagSeverity, GlyimDiagnostic};

pub fn assert_no_errors(diagnostics: &[GlyimDiagnostic]) {
    let errors: Vec<_> = diagnostics.iter().filter(|d| d.is_error()).collect();
    assert!(errors.is_empty(), "expected no errors, found {}", errors.len());
}

pub fn assert_has_errors(diagnostics: &[GlyimDiagnostic]) {
    assert!(diagnostics.iter().any(|d| d.is_error()), "expected at least one error");
}

pub fn assert_error_count(diagnostics: &[GlyimDiagnostic], expected: usize) {
    let actual = diagnostics.iter().filter(|d| d.is_error()).count();
    assert_eq!(actual, expected);
}

pub fn assert_diag_contains(diagnostics: &[GlyimDiagnostic], substring: &str) {
    assert!(
        diagnostics.iter().any(|d| d.message.contains(substring)),
        "expected diagnostic containing {:?}", substring
    );
}

pub fn assert_diag_code(diagnostics: &[GlyimDiagnostic], code: &glyim_diag::ErrorCode) {
    assert!(diagnostics.iter().any(|d| d.code == *code), "expected diagnostic with code {:?}", code);
}

pub fn assert_has_severity(diagnostics: &[GlyimDiagnostic], severity: DiagSeverity) {
    assert!(diagnostics.iter().any(|d| d.severity == severity));
}
```

```rust
// crates/glyim-test/src/assertions/span.rs
//! Span assertions for mock contexts.

use crate::mock::lower_ctx::SpanOp;
use glyim_span::Span;

pub fn assert_span_pushed(ops: &[SpanOp], expected: Span) {
    let found = ops.iter().any(|op| matches!(op, SpanOp::Push(s) if *s == expected));
    assert!(found, "expected span {:?} to have been pushed", expected);
}

pub fn assert_spans_balanced(ops: &[SpanOp]) {
    let depth = ops.iter().fold(0, |acc, op| match op {
        SpanOp::Push(_) => acc + 1,
        SpanOp::Pop => acc.saturating_sub(1),
    });
    assert_eq!(depth, 0, "unbalanced span operations");
}
```

---

## Snapshot

```rust
// crates/glyim-test/src/snapshot/mod.rs
//! Snapshot testing wrapping insta.

pub mod format;

pub fn snapshot_cst(name: &str, source: &str) {
    let result = glyim_frontend::parse_to_syntax(source, glyim_span::FileId::from_raw(0));
    let tree = format!("{:#?}", result.root);
    insta::with_settings!({ snapshot_suffix => name }, {
        insta::assert_snapshot!(tree);
    });
}

pub fn snapshot_mir(name: &str, ctx: &glyim_type::TyCtx, body: &glyim_mir::Body) {
    let formatted = format::format_mir_body(ctx, body);
    insta::with_settings!({ snapshot_suffix => name }, {
        insta::assert_snapshot!(formatted);
    });
}

pub fn snapshot_def_map(name: &str, def_map: &glyim_def_map::CrateDefMap) {
    let formatted = format::format_def_map(def_map);
    insta::with_settings!({ snapshot_suffix => name }, {
        insta::assert_snapshot!(formatted);
    });
}
```

```rust
// crates/glyim-test/src/snapshot/format.rs
//! Consistent formatting for all compiler data structures.

use glyim_diag::GlyimDiagnostic;
use glyim_type::TyCtx;

pub fn format_mir_body(ctx: &TyCtx, body: &glyim_mir::Body) -> String {
    let mut out = String::new();
    out.push_str(&format!("fn {}():\n", body.owner));
    out.push_str("  locals:\n");
    for (idx, local) in body.locals.iter_enumerated() {
        out.push_str(&format!(
            "    ${}: {} ({})\n",
            idx.to_raw(),
            glyim_type::PrintTy::new(local.ty, ctx),
            match local.mutability { Mutability::Mut => "mut", Mutability::Not => "imm" },
        ));
    }
    for (idx, block) in body.basic_blocks.iter_enumerated() {
        out.push_str(&format!("  bb{}:\n", idx.to_raw()));
        for stmt in &block.statements { out.push_str(&format!("    {:?}\n", stmt.kind)); }
        out.push_str(&format!("    -> {:?}\n", block.terminator.kind));
    }
    out
}

pub fn format_def_map(def_map: &glyim_def_map::CrateDefMap) -> String {
    let mut out = String::new();
    out.push_str(&format!("CrateDefMap (root: {:?}, krate: {:?})\n", def_map.root, def_map.krate));
    for (idx, module) in def_map.modules.iter_enumerated() {
        out.push_str(&format!("  module {:?}:\n", idx));
        out.push_str(&format!("    parent: {:?}\n", module.parent));
        out.push_str(&format!("    children: {}\n", module.children.len()));
    }
    out
}
```

---

## Property

```rust
// crates/glyim-test/src/property/mod.rs
pub mod arbitrary;
```

```rust
// crates/glyim-test/src/property/arbitrary.rs
//! Valid type generation through TyCtxMut.
//!
//! [F1] Never calls Ty::from_raw(). Uses TyCtxMut::mk_* methods.
//! [F2] Uses Ty::ERROR for error cases.
//! [F18] Separate TyVar, IntVar, FloatVar.
//!
//! Generator creates structurally valid types by allocating
//! through TyCtxMut, ensuring all Ty references are valid.

use glyim_core::primitives::*;
use glyim_type::*;
use rand::rngs::StdRng;
use rand::Rng;

pub struct Generator {
    rng: StdRng,
    max_depth: u32,
}

impl Generator {
    pub fn new(seed: u64) -> Self {
        Self { rng: StdRng::seed_from_u64(seed), max_depth: 4 }
    }

    pub fn with_max_depth(mut self, depth: u32) -> Self { self.max_depth = depth; self }

    /// Generate a valid Ty by allocating through TyCtxMut.
    pub fn generate_ty(&mut self, ctx: &mut TyCtxMut, depth: u32) -> Ty {
        if depth >= self.max_depth { return self.leaf_ty(ctx); }

        match self.rng.gen_range(0..8) {
            0 => ctx.mk_bool(),
            1 => ctx.mk_never(),
            2 => ctx.mk_unit(),
            3 => ctx.mk_ty(TyKind::Int(self.int_ty())),
            4 => ctx.mk_ty(TyKind::Uint(self.uint_ty())),
            5 => ctx.mk_ty(TyKind::Float(self.float_ty())),
            6 => {
                let inner = self.generate_ty(ctx, depth + 1);
                ctx.mk_ref(Region::Erased, inner, self.mutability())
            }
            7 => {
                let inner = self.generate_ty(ctx, depth + 1);
                ctx.mk_slice(inner)
            }
            _ => self.leaf_ty(ctx),
        }
    }

    fn leaf_ty(&mut self, ctx: &mut TyCtxMut) -> Ty {
        match self.rng.gen_range(0..4) {
            0 => ctx.mk_bool(),
            1 => ctx.mk_unit(),
            2 => ctx.mk_ty(TyKind::Int(self.int_ty())),
            _ => ctx.mk_ty(TyKind::Uint(self.uint_ty())),
        }
    }

    fn int_ty(&mut self) -> IntTy {
        match self.rng.gen_range(0..5) {
            0 => IntTy::I8, 1 => IntTy::I16, 2 => IntTy::I32,
            3 => IntTy::I64, _ => IntTy::Isize,
        }
    }
    fn uint_ty(&mut self) -> UintTy {
        match self.rng.gen_range(0..5) {
            0 => UintTy::U8, 1 => UintTy::U16, 2 => UintTy::U32,
            3 => UintTy::U64, _ => UintTy::Usize,
        }
    }
    fn float_ty(&mut self) -> FloatTy {
        if self.rng.gen_bool(0.5) { FloatTy::F32 } else { FloatTy::F64 }
    }
    fn mutability(&mut self) -> Mutability {
        if self.rng.gen_bool(0.5) { Mutability::Mut } else { Mutability::Not }
    }
}

/// Invariant: every Ty round-trips through ty_kind.
pub fn ty_roundtrip_invariant(ctx: &TyCtx) {
    for (ty, kind) in ctx.iter_types() {
        assert_eq!(ctx.ty_kind(ty), kind, "Round-trip failed for Ty({})", ty.to_raw());
    }
}

/// Invariant: sentinel types are at their expected positions.
pub fn sentinel_invariant(ctx: &TyCtx) {
    assert!(matches!(ctx.ty_kind(Ty::ERROR), TyKind::Error));
    assert!(matches!(ctx.ty_kind(Ty::NEVER), TyKind::Never));
    assert!(matches!(ctx.ty_kind(Ty::UNIT), TyKind::Unit));
    assert!(matches!(ctx.ty_kind(Ty::BOOL), TyKind::Bool));
}
```

---

## Fixtures

```rust
// crates/glyim-test/src/fixtures/mod.rs
pub mod builder;
pub use builder::*;
```

```rust
// crates/glyim-test/src/fixtures/builder.rs
//! Programmatic test input builders.

use glyim_core::interner::Interner;
use glyim_type::{TyCtx, TyCtxMut};

pub struct SourceBuilder { lines: Vec<String> }

impl SourceBuilder {
    pub fn new() -> Self { Self { lines: Vec::new() } }
    pub fn line(mut self, line: impl Into<String>) -> Self { self.lines.push(line.into()); self }
    pub fn empty(self) -> Self { self.line("") }
    pub fn fn_def(self, name: &str, params: &str, body: &str) -> Self {
        self.line(format!("fn {}({}) {{ {} }}", name, params, body))
    }
    pub fn mode(self, mode: &str) -> Self { self.line(format!("// test-mode: {}", mode)) }
    pub fn annotation(self, ann: &str) -> Self { self.line(format!("//~ {}", ann)) }
    pub fn build(self) -> String { self.lines.join("\n") }
}

impl Default for SourceBuilder { fn default() -> Self { Self::new() } }

pub struct TyCtxBuilder { interner: Option<Interner> }

impl TyCtxBuilder {
    pub fn new() -> Self { Self { interner: None } }
    pub fn with_interner(mut self, interner: Interner) -> Self { self.interner = Some(interner); self }
    pub fn build_mut(self) -> TyCtxMut {
        let interner = self.interner.unwrap_or_else(Interner::new);
        TyCtxMut::new(interner)
    }
    pub fn build(self) -> TyCtx { self.build_mut().freeze() }
}

impl Default for TyCtxBuilder { fn default() -> Self { Self::new() } }
```

---

## Integration Test Examples

```rust
// tests/compile_fail_tests.rs
use glyim_test::harness::{TestRunner, TestMode};

#[test]
fn compile_fail() {
    let plan = TestRunner::new("tests/compile-fail")
        .mode(TestMode::CompileFail)
        .parallel(true)
        .build()
        .expect("failed to discover tests");
    plan.run();
}
```

```rust
// tests/ui_tests.rs
use glyim_test::harness::{TestRunner, TestMode};

#[test]
fn ui_tests() {
    let plan = TestRunner::new("tests/ui")
        .mode(TestMode::Ui)
        .build()
        .expect("failed to discover tests");
    plan.run();
}
```

```rust
// tests/unit_tests.rs
use glyim_test::*;
use glyim_core::primitives::*;

#[test]
fn test_ty_assert_is_int() {
    let (ctx, ty) = with_fresh_ty_ctx(|ctx| ctx.mk_int(IntTy::I32));
    assert_ty(&ctx, ty).is_int(IntTy::I32);
}

#[test]
fn test_ty_assert_chained_ref() {
    let mut ctx_mut = test_ty_ctx();
    let inner = ctx_mut.mk_bool();
    let ref_ty = ctx_mut.mk_ref(glyim_type::Region::Erased, inner, Mutability::Mut);
    let ctx = ctx_mut.freeze();
    assert_ty(&ctx, ref_ty).is_ref(Mutability::Mut).is_bool();
}

#[test]
fn test_mock_solver() {
    let mut solver = mock::MockSolver::new()
        .respond_for_any(glyim_solve::SolverResult::Proven);
    let ctx = test_frozen_ty_ctx();
    let result = solver.can_prove(&ctx, &glyim_type::TraitPredicate {
        trait_ref: glyim_type::TraitRef {
            def_id: glyim_core::def_id::TraitDefId::from_raw(0),
            substs: glyim_type::Substitution::from_raw(0, 0),
        },
        polarity: glyim_type::ImplPolarity::Positive,
    });
    assert_eq!(result, glyim_solve::SolverResult::Proven);
    solver.assert_call_count(1);
}

#[test]
fn test_source_builder() {
    let source = fixtures::SourceBuilder::new()
        .mode("compile-fail")
        .empty()
        .fn_def("main", "", r#"let x: i32 = "hello""#)
        .annotation("ERROR mismatched types")
        .build();
    assert!(source.contains("fn main"));
    assert!(source.contains("//~ ERROR"));
}

#[test]
fn test_property_generator_valid_types() {
    let mut ctx = test_ty_ctx();
    let mut gen = property::Generator::new(42);
    let ty = gen.generate_ty(&mut ctx, 0);
    let frozen = ctx.freeze();
    assert!(
        !matches!(frozen.ty_kind(ty), TyKind::Error),
        "Generator should not produce errors by default"
    );
    property::ty_roundtrip_invariant(&frozen);
    property::sentinel_invariant(&frozen);
}

#[test]
fn test_sentinel_constants() {
    // [F2] Public sentinels work without from_raw
    let ctx = test_frozen_ty_ctx();
    assert!(matches!(ctx.ty_kind(Ty::ERROR), TyKind::Error));
    assert!(matches!(ctx.ty_kind(Ty::NEVER), TyKind::Never));
    assert!(matches!(ctx.ty_kind(Ty::UNIT), TyKind::Unit));
    assert!(matches!(ctx.ty_kind(Ty::BOOL), TyKind::Bool));
}
```

---

## Example Test File

```gly
// tests/compile-fail/type_mismatch.g
// test-mode: compile-fail
// error-pattern: mismatched types

fn main() {
    let x: i32 = "hello"; //~ ERROR mismatched types
    let y: bool = 42;       //~ ERROR mismatched types
                           //~| NOTE expected `bool`
}
```

---

## Summary: Infrastructure Leverage Map

| Infrastructure Change | How `glyim-test` Leverages It |
|---|---|
| **[F1]** No `IdxLike for Ty` | Never calls `Ty::from_raw()`. Uses `Ty::ERROR` etc. or `TyCtxMut` methods |
| **[F2]** Public sentinel constants | All dummy bodies use `Ty::ERROR`, never `Ty::from_raw(0)` |
| **[F4]** `TypeLookup` trait | `TyAssert`, `PrintTy`, `MirAssert` all generic over `TypeLookup` |
| **[F13]** No Salsa | `TestDbBuilder` creates simple `Database` |
| **[F16]** `DiagSink` logging | Executor creates `DiagSink::new()` with default logging |
| **[F18]** Separate IntVar/FloatVar | Property generator and mocks use correct types |
| Pipeline delegation | Executor comments note delegation to `Pipeline::compile_file` |
| `.g` extension | Collector uses `.g` exclusively |
| Strict parsing | Config, severity, and mode names are strictly validated |
| Single-pass annotations | No double-matching of `//~|` |
| Exact match by default | `//~~` for fuzzy, otherwise exact line comparison |
| Builder → Plan separation | `TestRunner::build()` returns `Result<TestPlan, String>` |
| Environment variables | `GLYIM_BLESS` and `GLYIM_TEST_SHOW_OUTPUT` |
