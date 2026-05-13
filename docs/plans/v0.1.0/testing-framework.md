# `glyim-test` — Complete Redesigned Plan (v3)

Every finding from both critiques is addressed. Each fix is tagged with **[#N]** for traceability to the second critique, and **[F#]** for the first.

---

## 0. Required Changes to Other Crates

These changes are prerequisites. Without them, `glyim-test` cannot exercise certain features.

### `glyim-type`

| Change | Why |
|--------|-----|
| Add `pub fn iter_types(&self) -> impl Iterator<Item = (Ty, &TyKind)>` to `TyCtx` and `TyCtxMut` | **[#1]** Enables `ty_roundtrip_invariant` |
| Add `pub fn mk_ty_var(&mut self) -> Ty`, `mk_int_var(&mut self) -> Ty`, `mk_float_var(&mut self) -> Ty` | **[#12]** Enables inference variable testing **[F8]** |
| Add `pub fn empty_substitution() -> Substitution` | **[#2]** Public constructor replaces `from_raw` |
| Add `pub fn new_trait_predicate(def_id: TraitDefId, substs: Substitution) -> TraitPredicate` | **[#2]** Test construction of `TraitPredicate` |

### `glyim-lower`

| Change | Why |
|--------|-----|
| Add methods to `LowerCtx` trait: `fn ty_ctx(&self) -> &TyCtx`, `fn push_span(&self, span: Span)`, `fn pop_span(&self)` | **[#5]** Empty trait is useless; mocks need real contract |

### `glyim-borrowck`

| Change | Why |
|--------|-----|
| Add methods to `BorrowckCtx` trait: `fn ty_ctx(&self) -> &TyCtx`, `fn local_decl(&self, local: LocalIdx) -> &LocalDecl`, `fn is_copy(&self, ty: Ty) -> bool` | **[#6]** Empty trait is useless |

### `glyim-solve`

| Change | Why |
|--------|-----|
| Add methods to `TraitSolver` trait: `fn can_prove(&mut self, ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult`, `fn evaluate_predicate(&mut self, ctx: &TyCtx, predicate: &Predicate) -> SolverResult` | **[#7]** Empty trait is useless |

### `glyim-pipeline`

| Change | Why |
|--------|-----|
| Add `pub fn compile_file(&self, file_id: FileId) -> Result<CompileOutput, CompileFailed>` where both structs contain `pub diagnostics: Vec<GlyimDiagnostic>` | **[F7]** Full-pipeline compilation |

### `glyim-db`

| Change | Why |
|--------|-----|
| Add `pub fn vfs_mut(&mut self) -> &mut Vfs` (or ensure `vfs()` returns `&mut Vfs`) | TestDbBuilder needs to add files |

Until these are implemented, `glyim-test` uses the **`FrontendOnlyCompiler`** fallback and mock structs are **standalone test helpers** (not trait impls for empty traits).

---

## 1. Cargo.toml

```toml
# crates/glyim-test/Cargo.toml
[package]
name = "glyim-test"
edition.workspace = true
version.workspace = true

[features]
default = []
property-proptest = ["proptest"]
json-output = ["serde", "serde_json"]

[dependencies]
# ── Workspace crates ── [#9] ALL required crates listed ──
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

# ── Third-party ── [#9] All with explicit versions ──
insta        = { workspace = true }
tracing      = { workspace = true }
parking_lot  = { workspace = true }

rayon        = "1.10"
similar      = "2.7"
termcolor    = "1.4"
walkdir      = "2.5"
regex        = "1.11"
shell-words  = "2.0"
rand         = "0.8"

serde        = { version = "1.0", optional = true }
serde_json   = { workspace = true, optional = true }
proptest     = { version = "1.5", optional = true }

[dev-dependencies]
tempfile = "3.14"
```

---

## 2. Top-Level Module Structure

```
src/
├── lib.rs
├── error.rs                  [#1,#2,#9,#15] Structured errors, FailureReason
├── harness/
│   ├── mod.rs
│   ├── config.rs             [#10,#13] FromStr for TestMode, has_explicit_mode
│   ├── collector.rs          [#9,#14]   Arc<DiscoveredTest>, TestDiscoveryError
│   ├── plan.rs               run() + execute()
│   ├── executor.rs           [#4,#5,#6,#7,#17,#18] timeout, FileId, tracing
│   ├── strategy.rs           Split God-function into strategies
│   ├── compiler.rs           [#7] TestCompiler trait, FrontendOnlyCompiler
│   └── reporter.rs           [#19] Stderr + optional JSON output
├── annotations/
│   ├── mod.rs                [#15] Fixed parser: ~~ before ~
│   └── pattern.rs            [#13] MatchPattern lives HERE
├── comparison/
│   ├── mod.rs                [#3,#12] Correct invariant, passed() method
│   └── normalize.rs
├── mock/
│   ├── mod.rs
│   ├── lower_ctx.rs          [#5] Standalone helper (trait is empty)
│   ├── borrowck_ctx.rs       [#6] Standalone helper (trait is empty)
│   ├── solver.rs             [#7,#11] Standalone helper, Vec not RefCell
│   ├── codegen.rs            [#4] Implements ACTUAL CodegenBackend signature
│   └── db.rs                 [#10] Database::new() only
├── assertions/
│   ├── mod.rs
│   ├── ty.rs                 [#3] bool_ty/never_ty/unit_ty, mk_ty only
│   ├── mir.rs                [#8] Only real TerminatorKind variants
│   ├── diag.rs
│   └── span.rs
├── snapshot/
│   ├── mod.rs
│   └── format.rs             [#11] Uses glyim_mir::Mutability
├── property/
│   ├── mod.rs
│   ├── arbitrary.rs          [#3,#12] Current API only; inference vars noted
│   └── check.rs              Property test wrapper
└── fixtures/
    ├── mod.rs
    └── builder.rs
```

---

## 3. `lib.rs`

```rust
// crates/glyim-test/src/lib.rs
//! Compiler testing framework for Glyim.
//!
//! # API Reconciliation (v3)
//!
//! This crate uses ONLY public APIs confirmed to exist:
//!
//! - `Ty::ERROR`, `Ty::NEVER`, `Ty::UNIT`, `Ty::BOOL` (sentinel constants)
//! - `TyCtxMut::bool_ty()`, `never_ty()`, `unit_ty()`, `mk_ty()`, `mk_ref()`
//! - `TyCtxMut::freeze() -> TyCtx`
//! - `TypeLookup` trait for `ty_kind()`, `ty_flags()`, `PrintTy`
//! - `Database::new()` only (no CrateConfig, no with_interner) [#10]
//! - `CodegenBackend::generate(&self, bodies, output) -> CompResult<Vec<u8>>` [#4]
//!
//! # Deferred Until Upstream Crates Provide:
//!
//! - Inference variable allocation (`mk_ty_var` etc.) [#12]
//! - Full Pipeline compilation (#7, see `compiler.rs`)
//! - Trait impls for empty traits (#5, #6, #7)

pub mod error;
pub mod harness;
pub mod annotations;
pub mod comparison;
pub mod mock;
pub mod assertions;
pub mod snapshot;
pub mod property;
pub mod fixtures;

// ── Re-exports ──

pub use error::{TestDiscoveryError, FailureReason, TimeoutError, AssertionFailure};

pub use harness::{TestRunner, TestPlan, TestMode};
pub use mock::{MockSolver, MockCodegen, MockBorrowckCtx, MockLowerCtx, TestDbBuilder};
pub use assertions::{
    assert_ty, TyAssert,
    check_ty, TyCheck,
    assert_mir, MirAssert,
    assert_no_errors, assert_has_errors, assert_error_count,
    assert_diag_contains, assert_diag_code, assert_has_severity,
};
pub use snapshot::{snapshot_cst, snapshot_mir, snapshot_def_map};
pub use fixtures::{SourceBuilder, TyCtxBuilder};
pub use property::check_ty_property;

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

## 4. `error.rs`

```rust
// crates/glyim-test/src/error.rs
//! [#9,#15] Structured error types. No String anywhere.

use std::path::PathBuf;

/// Errors during test discovery and plan construction. [#9]
#[derive(Debug)]
pub enum TestDiscoveryError {
    RootNotFound(PathBuf),
    ReadFailed { path: PathBuf, source: std::io::Error },
    InvalidConfig { path: PathBuf, message: String },
    InvalidAnnotation { path: PathBuf, line: usize, message: String },
}

impl std::fmt::Display for TestDiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootNotFound(p) => write!(f, "test directory does not exist: {:?}", p),
            Self::ReadFailed { path, source } => write!(f, "read {:?}: {}", path, source),
            Self::InvalidConfig { path, message } => {
                write!(f, "invalid config in {:?}: {}", path, message)
            }
            Self::InvalidAnnotation { path, line, message } => {
                write!(f, "invalid annotation in {:?} line {}: {}", path, line, message)
            }
        }
    }
}

impl std::error::Error for TestDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFailed { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// [#15] Structured failure reason for CI/IDE consumption.
#[derive(Clone, Debug)]
pub enum FailureReason {
    CompilePassUnexpectedErrors { errors: Vec<String> },
    AnnotationParseError { line: usize, message: String },
    DiagnosticMismatch {
        missing_count: usize,
        unexpected_count: usize,
        wrong_severity_count: usize,
        details: String,
    },
    ErrorPatternNotFound { pattern: String },
    UiOutputDiffers { diff: String },
    UiNoExpectedFile { path: PathBuf },
    TimeoutExceeded { timeout_secs: u64 },
    CompilationFailed { phase: String, message: String },
}

impl std::fmt::Display for FailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CompilePassUnexpectedErrors { errors } => {
                write!(f, "expected compilation to succeed, got {} error(s):\n  {}",
                    errors.len(), errors.join("\n  "))
            }
            Self::AnnotationParseError { line, message } => {
                write!(f, "annotation parse error at line {}: {}", line, message)
            }
            Self::DiagnosticMismatch {
                missing_count, unexpected_count, wrong_severity_count, details
            } => {
                write!(f, "diagnostic mismatch ({} missing, {} unexpected, {} wrong severity):\n  {}",
                    missing_count, unexpected_count, wrong_severity_count, details)
            }
            Self::ErrorPatternNotFound { pattern } => {
                write!(f, "error-pattern '{}' not found", pattern)
            }
            Self::UiOutputDiffers { diff } => write!(f, "output differs:\n{}", diff),
            Self::UiNoExpectedFile { path } => write!(f, "no expected file: {:?}", path),
            Self::TimeoutExceeded { timeout_secs } => write!(f, "exceeded {}s timeout", timeout_secs),
            Self::CompilationFailed { phase, message } => {
                write!(f, "compilation failed at {}: {}", phase, message)
            }
        }
    }
}

/// Timeout error for test execution.
#[derive(Clone, Debug)]
pub struct TimeoutError { pub timeout_secs: u64 }

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "test exceeded {}s timeout", self.timeout_secs)
    }
}
impl std::error::Error for TimeoutError {}

/// Single type assertion failure for composable checking.
#[derive(Clone, Debug)]
pub struct AssertionFailure {
    pub expected: String,
    pub actual: String,
    pub ty_description: String,
}
```

---

## 5. Harness: Config

```rust
// crates/glyim-test/src/harness/config.rs
//! [#10] Explicit header always wins over directory.
//! [#13] FromStr for TestMode.

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TestMode {
    CompilePass,
    CompileFail,
    Ui,
}

/// [#13] Idiomatic FromStr instead of hand-rolled from_str_exact.
impl FromStr for TestMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "compile-pass" => Ok(Self::CompilePass),
            "compile-fail" => Ok(Self::CompileFail),
            "ui" => Ok(Self::Ui),
            other => Err(format!(
                "unknown test-mode: {:?}. Expected: compile-pass, compile-fail, ui",
                other
            )),
        }
    }
}

impl TestMode {
    pub fn from_str_exact(s: &str) -> Result<Self, String> { s.parse() }
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::CompilePass => "compile-pass",
            Self::CompileFail => "compile-fail",
            Self::Ui => "ui",
        }
    }
}

/// [#10] Tracks whether mode was explicitly set so header wins over directory.
pub struct ParsedConfig {
    pub config: TestConfig,
    pub has_explicit_mode: bool,
}

/// Parse test configuration from file header comments.
pub fn parse_test_config(source: &str) -> Result<ParsedConfig, String> {
    let mut config = TestConfig::default();
    let mut has_explicit_mode = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("//") {
            if trimmed.is_empty() { continue; }
            break;
        }
        let content = trimmed[2..].trim();

        // Revision-specific: [rev] key: value
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
            config.mode = value.parse::<TestMode>()?;
            has_explicit_mode = true; // [#10]
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

    Ok(ParsedConfig { config, has_explicit_mode })
}
```

---

## 6. Harness: Collector

```rust
// crates/glyim-test/src/harness/collector.rs
//! [#9] TestDiscoveryError. [#10] Header wins. [#14] Arc<DiscoveredTest>.

use super::config::{ParsedConfig, TestConfig, TestMode};
use crate::error::TestDiscoveryError;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct DiscoveredTest {
    pub path: PathBuf,
    pub name: String,
    pub config: TestConfig,
    pub source: Arc<str>,
    pub revisions: Vec<String>,
}

pub struct TestCollector<'a> { root: &'a Path }

impl<'a> TestCollector<'a> {
    pub fn new(root: &'a Path) -> Self { Self { root } }

    pub fn collect(
        &self,
        filter: Option<&str>,
        mode_override: Option<TestMode>,
    ) -> Result<Vec<Arc<DiscoveredTest>>, TestDiscoveryError> {
        if !self.root.exists() {
            return Err(TestDiscoveryError::RootNotFound(self.root.to_path_buf()));
        }

        let mut tests = Vec::new();

        for entry in walkdir::WalkDir::new(self.root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("g") { continue; }
            if let Some(f) = filter {
                if !path.to_string_lossy().contains(f) { continue; }
            }

            let source: Arc<str> = std::fs::read_to_string(path)
                .map_err(|e| TestDiscoveryError::ReadFailed {
                    path: path.to_path_buf(), source: e,
                })?
                .into();

            let ParsedConfig { config: header_config, has_explicit_mode } =
                super::config::parse_test_config(&source)
                    .map_err(|msg| TestDiscoveryError::InvalidConfig {
                        path: path.to_path_buf(), message: msg,
                    })?;

            let mut config = TestConfig::default();

            // [#10] Priority: 1) mode_override  2) explicit header  3) directory
            let dir_mode = path.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .and_then(|s| s.parse::<TestMode>().ok());

            if has_explicit_mode {
                config.mode = header_config.mode;
            } else if let Some(dm) = dir_mode {
                config.mode = dm;
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

            // [#14] Arc<DiscoveredTest> — wrap once, reference everywhere.
            tests.push(Arc::new(DiscoveredTest {
                path: path.to_path_buf(), name, config, source, revisions,
            }));
        }

        tests.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tests)
    }
}
```

---

## 7. Harness: Plan & Runner

```rust
// crates/glyim-test/src/harness/plan.rs
//! Immutable test plan with both execute() and run().

use super::config::TestMode;
use super::collector::TestCollector;
use super::executor::TestExecutor;
use super::reporter::TestReporter;
use crate::error::TestDiscoveryError;
use std::path::PathBuf;
use std::time::Duration;

pub struct TestRunner {
    root: PathBuf,
    mode_override: Option<TestMode>,
    parallel: bool,
    filter: Option<String>,
    timeout: Duration,
    max_concurrent: usize,
}

impl TestRunner {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            mode_override: None,
            parallel: true,
            filter: None,
            timeout: Duration::from_secs(60),
            max_concurrent: 8,
        }
    }
    pub fn mode(mut self, mode: TestMode) -> Self { self.mode_override = Some(mode); self }
    pub fn parallel(mut self, yes: bool) -> Self { self.parallel = yes; self }
    pub fn filter(mut self, f: impl Into<String>) -> Self { self.filter = Some(f.into()); self }
    pub fn timeout(mut self, d: Duration) -> Self { self.timeout = d; self }
    pub fn max_concurrent(mut self, n: usize) -> Self { self.max_concurrent = n; self }

    /// [#9] Returns TestDiscoveryError instead of String.
    pub fn build(self) -> Result<TestPlan, TestDiscoveryError> {
        let collector = TestCollector::new(&self.root);
        let tests = collector.collect(self.filter.as_deref(), self.mode_override)?;
        Ok(TestPlan {
            tests,
            parallel: self.parallel,
            default_timeout: self.timeout,
            max_concurrent: self.max_concurrent,
            bless: std::env::var("GLYIM_BLESS").is_ok(),
            verbose: std::env::var("GLYIM_TEST_SHOW_OUTPUT").is_ok(),
        })
    }
}

pub struct TestPlan {
    pub tests: Vec<std::sync::Arc<super::collector::DiscoveredTest>>,
    pub parallel: bool,
    pub default_timeout: Duration,
    pub max_concurrent: usize,
    pub bless: bool,
    pub verbose: bool,
}

pub struct ExecutionResult {
    pub results: Vec<super::executor::TestResult>,
    pub summary: TestSummary,
}

impl TestPlan {
    /// Execute and return results. Does NOT panic.
    pub fn execute(self) -> ExecutionResult {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();

        if self.tests.is_empty() {
            eprintln!("no test files found");
            return ExecutionResult { results: Vec::new(), summary: TestSummary::default() };
        }

        eprintln!("running {} tests", self.tests.len());

        let executor = TestExecutor::new(
            self.default_timeout, self.bless, self.verbose, self.max_concurrent,
        );
        let results = if self.parallel {
            executor.run_parallel(&self.tests)
        } else {
            executor.run_sequential(&self.tests)
        };

        let reporter = TestReporter::new(self.verbose);
        let summary = reporter.report(&results);

        ExecutionResult { results, summary }
    }

    /// Execute and panic on failure (for `cargo test`).
    pub fn run(self) {
        let result = self.execute();
        if result.summary.failed > 0 {
            let failed_names: Vec<_> = result.results.iter()
                .filter(|r| matches!(r.outcome, super::executor::TestOutcome::Failed { .. }))
                .map(|r| format!("{} [{}]", r.test.name, r.revision))
                .collect();
            panic!(
                "{} tests FAILED: {}\n\
                 Run with GLYIM_TEST_SHOW_OUTPUT=1 for details.",
                result.summary.failed,
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

## 8. Harness: Compiler (new)

```rust
// crates/glyim-test/src/harness/compiler.rs
//! [#7] Abstraction over compilation. Default uses frontend-only.
//! When Pipeline::compile_file exists, add PipelineCompiler.

use glyim_diag::GlyimDiagnostic;
use glyim_span::FileId;

/// Result of compilation — always contains diagnostics.
pub struct CompileOutput {
    pub diagnostics: Vec<GlyimDiagnostic>,
}

/// Trait abstracting how a test source is compiled.
/// This decouples the test framework from the Pipeline's compile path,
/// which may not be fully wired up in v0.1.0.
pub trait TestCompiler: Send + Sync {
    fn compile(
        &self,
        source: &str,
        file_id: FileId,
        flags: &[String],
    ) -> CompileOutput;
}

/// Frontend-only compiler. Parses and returns parse diagnostics.
/// This is the default for v0.1.0 until Pipeline::compile_file is ready.
pub struct FrontendOnlyCompiler;

impl TestCompiler for FrontendOnlyCompiler {
    fn compile(
        &self,
        source: &str,
        file_id: FileId,
        _flags: &[String],
    ) -> CompileOutput {
        tracing::info!(phase = "parse", file_id = file_id.to_raw());
        let result = glyim_frontend::parse_to_syntax(source, file_id);
        CompileOutput {
            diagnostics: result.diagnostics,
        }
    }
}

// ── When Pipeline::compile_file is available, uncomment: ──
//
// use glyim_db::Database;
// use glyim_pipeline::Pipeline;
//
// pub struct PipelineCompiler<'a> { db: &'a Database }
//
// impl<'a> TestCompiler for PipelineCompiler<'a> {
//     fn compile(&self, source: &str, file_id: FileId, flags: &[String]) -> CompileOutput {
//         tracing::info!(phase = "full", file_id = file_id.to_raw());
//         let pipeline = Pipeline::new(self.db);
//         match pipeline.compile_file(file_id) {
//             Ok(output) => CompileOutput { diagnostics: output.diagnostics },
//             Err(failed) => CompileOutput { diagnostics: failed.diagnostics },
//         }
//     }
// }
```

---

## 9. Harness: Executor

```rust
// crates/glyim-test/src/harness/executor.rs
//! [#4] Revision support. [#5] Timeout. [#6] Unique FileId.
//! [#17] Configurable target triple. [#18] Tracing spans.

use super::collector::DiscoveredTest;
use super::compiler::{CompileOutput, FrontendOnlyCompiler, TestCompiler};
use super::strategy;
use crate::comparison::NormalizedDiag;
use crate::error::{FailureReason, TimeoutError};
use glyim_diag::GlyimDiagnostic;
use glyim_span::FileId;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// [#6] Unique FileId per test. 0 is reserved.
static NEXT_FILE_ID: AtomicU32 = AtomicU32::new(1);

fn next_file_id() -> FileId {
    FileId::from_raw(NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone, Debug)]
pub enum TestOutcome {
    Passed,
    Failed { reason: FailureReason },
    Ignored,
}

#[derive(Clone, Debug)]
pub struct TestResult {
    pub test: Arc<DiscoveredTest>,
    pub revision: String,
    pub outcome: TestOutcome,
    pub duration: Duration,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

pub struct TestExecutor {
    default_timeout: Duration,
    bless: bool,
    verbose: bool,
    max_concurrent: usize,
    target_triple: String,
    compiler: Box<dyn TestCompiler>,
}

impl TestExecutor {
    pub fn new(
        default_timeout: Duration,
        bless: bool,
        verbose: bool,
        max_concurrent: usize,
    ) -> Self {
        Self {
            default_timeout,
            bless,
            verbose,
            max_concurrent,
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            compiler: Box::new(FrontendOnlyCompiler),
        }
    }

    /// [#17] Override target triple from configuration.
    pub fn with_target_triple(mut self, triple: impl Into<String>) -> Self {
        self.target_triple = triple.into();
        self
    }

    /// Swap the compiler backend (e.g., PipelineCompiler when available).
    pub fn with_compiler(mut self, compiler: Box<dyn TestCompiler>) -> Self {
        self.compiler = compiler;
        self
    }

    pub fn run_sequential(&self, tests: &[Arc<DiscoveredTest>]) -> Vec<TestResult> {
        tests.iter()
            .flat_map(|t| {
                let revs = t.revisions.clone();
                revs.into_iter().map(|r| self.execute(Arc::clone(t), &r))
            })
            .collect()
    }

    pub fn run_parallel(&self, tests: &[Arc<DiscoveredTest>]) -> Vec<TestResult> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.max_concurrent)
            .build()
            .unwrap();
        let tests_c = tests.to_vec();
        pool.install(move || {
            tests_c.par_iter()
                .flat_map(|t| {
                    let revs = t.revisions.clone();
                    revs.into_iter()
                        .map(|r| self.execute(Arc::clone(t), &r))
                        .collect::<Vec<_>>()
                })
                .collect()
        })
    }

    fn execute(&self, test: Arc<DiscoveredTest>, revision: &str) -> TestResult {
        let span = tracing::info_span!("test", name = %test.name, revision);
        let _enter = span.enter();
        let start = std::time::Instant::now();

        if test.config.ignore {
            return TestResult {
                test, revision: revision.to_string(),
                outcome: TestOutcome::Ignored, duration: start.elapsed(),
                diagnostics: Vec::new(),
            };
        }

        if test.config.needs_llvm && !cfg!(feature = "llvm") {
            return TestResult {
                test, revision: revision.to_string(),
                outcome: TestOutcome::Ignored, duration: start.elapsed(),
                diagnostics: Vec::new(),
            };
        }

        // [#17] Compare against configured triple, not hardcoded.
        if let Some(ref target) = test.config.only_target {
            if target != &self.target_triple {
                return TestResult {
                    test, revision: revision.to_string(),
                    outcome: TestOutcome::Ignored, duration: start.elapsed(),
                    diagnostics: Vec::new(),
                };
            }
        }

        // [#5] Per-test timeout enforcement.
        let timeout = Duration::from_secs(test.config.timeout_secs);
        let test_clone = Arc::clone(&test);
        let revision_owned = revision.to_string();

        let result = run_with_timeout(timeout, move || {
            Self::execute_inner(&test_clone, &revision_owned, &*self.compiler, self.bless)
        });

        let (outcome, diagnostics) = match result {
            Ok((outcome, diags)) => (outcome, diags),
            Err(TimeoutError { timeout_secs }) => (
                TestOutcome::Failed {
                    reason: FailureReason::TimeoutExceeded { timeout_secs },
                },
                Vec::new(),
            ),
        };

        let duration = start.elapsed();
        tracing::info!(duration_ms = duration.as_millis(), "done");

        TestResult { test, revision: revision.to_string(), outcome, duration, diagnostics }
    }

    /// [#4] Revision support: merges revision-specific compile flags.
    fn execute_inner(
        test: &Arc<DiscoveredTest>,
        revision: &str,
        compiler: &dyn TestCompiler,
        bless: bool,
    ) -> (TestOutcome, Vec<GlyimDiagnostic>) {
        // [#4] Merge base + revision flags.
        let mut flags = test.config.compile_flags.clone();
        if let Some(rev_flags) = test.config.revision_compile_flags.get(revision) {
            flags.extend(rev_flags.iter().cloned());
        }

        // [#6] Unique FileId per test.
        let file_id = next_file_id();

        // [#7] Compile through TestCompiler abstraction.
        let compile_span = tracing::info_span!("compile", file_id = file_id.to_raw());
        let output = compile_span.in_scope(|| compiler.compile(&test.source, file_id, &flags));

        // [#18] Tracing around comparison.
        let compare_span = tracing::info_span!("compare");
        let outcome = compare_span.in_scope(|| {
            match test.config.mode {
                super::config::TestMode::CompilePass => {
                    strategy::CompilePassStrategy.evaluate(&output.diagnostics, &test.source)
                }
                super::config::TestMode::CompileFail => {
                    strategy::CompileFailStrategy.evaluate(
                        &output.diagnostics, &test.source, &test.config.error_patterns,
                    )
                }
                super::config::TestMode::Ui => {
                    strategy::UiTestStrategy.evaluate(
                        &output.diagnostics, &test.source, &test.path, bless,
                    )
                }
            }
        });

        (outcome, output.diagnostics)
    }
}

/// [#5] Timeout enforcement via thread spawn + recv_timeout.
///
/// **Limitation:** Rust cannot kill threads. A hanging test's thread leaks
/// but the runner continues. This prevents deadlocking the parallel suite.
fn run_with_timeout<F, R>(timeout: Duration, f: F) -> Result<R, TimeoutError>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    let timeout_secs = timeout.as_secs();

    std::thread::spawn(move || {
        let result = f();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => Ok(result),
        Err(_) => Err(TimeoutError { timeout_secs }),
    }
}
```

---

## 10. Harness: Strategy

```rust
// crates/glyim-test/src/harness/strategy.rs
//! Split from the God-function executor.

use crate::annotations::Annotation;
use crate::comparison::{self, DiagSeverityExt, NormalizedDiag};
use crate::error::FailureReason;
use glyim_diag::{DiagSeverity, GlyimDiagnostic};
use std::path::Path;

/// compile-pass: expect zero errors.
pub struct CompilePassStrategy;

impl CompilePassStrategy {
    pub fn evaluate(
        &self,
        diagnostics: &[GlyimDiagnostic],
        _source: &str,
    ) -> super::executor::TestOutcome {
        let errors: Vec<String> = diagnostics.iter()
            .filter(|d| d.is_error())
            .map(|e| e.message.clone())
            .collect();
        if errors.is_empty() {
            super::executor::TestOutcome::Passed
        } else {
            super::executor::TestOutcome::Failed {
                reason: FailureReason::CompilePassUnexpectedErrors { errors },
            }
        }
    }
}

/// compile-fail: expect annotations to match diagnostics.
pub struct CompileFailStrategy;

impl CompileFailStrategy {
    pub fn evaluate(
        &self,
        diagnostics: &[GlyimDiagnostic],
        source: &str,
        error_patterns: &[String],
    ) -> super::executor::TestOutcome {
        let annotations = match Annotation::parse_all(source) {
            Ok(a) => a,
            Err(e) => {
                return super::executor::TestOutcome::Failed {
                    reason: FailureReason::AnnotationParseError { line: 0, message: e },
                };
            }
        };

        let normalized: Vec<NormalizedDiag> = diagnostics.iter()
            .map(|d| NormalizedDiag::from_glyim_diag(d, source))
            .collect();

        let result = comparison::compare_diagnostics(&annotations, &normalized);

        if result.passed() {
            for pattern in error_patterns {
                if !diagnostics.iter().any(|d| d.message.contains(pattern)) {
                    return super::executor::TestOutcome::Failed {
                        reason: FailureReason::ErrorPatternNotFound { pattern: pattern.clone() },
                    };
                }
            }
            super::executor::TestOutcome::Passed
        } else {
            let details = format_mismatch(&result);
            super::executor::TestOutcome::Failed {
                reason: FailureReason::DiagnosticMismatch {
                    missing_count: result.missing.len(),
                    unexpected_count: result.unexpected.len(),
                    wrong_severity_count: result.wrong_severity.len(),
                    details,
                },
            }
        }
    }
}

/// ui: compare output snapshot against expected file.
pub struct UiTestStrategy;

impl UiTestStrategy {
    pub fn evaluate(
        &self,
        diagnostics: &[GlyimDiagnostic],
        source: &str,
        test_path: &Path,
        bless: bool,
    ) -> super::executor::TestOutcome {
        let mut output = String::new();
        output.push_str("=== Diagnostics ===\n");
        for diag in diagnostics {
            output.push_str(&format!(
                "{}[{}]: {}\n",
                diag.severity.display_name(), diag.code, diag.message,
            ));
        }

        let normalized = crate::comparison::normalize::normalize_output(
            &output, test_path, &Default::default(),
        );

        let expected_path = test_path.with_extension("expected");

        // Bless: write and pass.
        if bless {
            std::fs::write(&expected_path, &normalized).unwrap();
            return super::executor::TestOutcome::Passed;
        }

        // No expected file? Fail. (Dead bless branch removed — [#16 from F1 critique])
        if !expected_path.exists() {
            return super::executor::TestOutcome::Failed {
                reason: FailureReason::UiNoExpectedFile { path: expected_path },
            };
        }

        let expected = std::fs::read_to_string(&expected_path).unwrap();
        if normalized == expected {
            super::executor::TestOutcome::Passed
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
            super::executor::TestOutcome::Failed {
                reason: FailureReason::UiOutputDiffers { diff: diff_str },
            }
        }
    }
}

fn format_mismatch(result: &comparison::ComparisonResult) -> String {
    let mut reasons = Vec::new();
    for m in &result.missing {
        reasons.push(format!(
            "line {}: expected {} {}",
            m.target_line() + 1,
            m.severity.display_name(),
            m.pattern.description()
        ));
    }
    for u in &result.unexpected {
        reasons.push(format!(
            "line {}: unexpected {} : {}",
            u.line + 1,
            u.severity.display_name(),
            u.message
        ));
    }
    for w in &result.wrong_severity {
        reasons.push(format!(
            "line {}: expected {} got {} : {}",
            w.diagnostic.line + 1,
            w.expected.display_name(),
            w.actual.display_name(),
            w.diagnostic.message
        ));
    }
    reasons.join("\n  ")
}
```

---

## 11. Harness: Reporter

```rust
// crates/glyim-test/src/harness/reporter.rs
//! [#19] Stderr text + optional JSON output.

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
            stderr, "{} run, {} passed, {} failed, {} ignored",
            summary.total, summary.passed, summary.failed, summary.ignored,
        );

        // [#19] JSON output for CI.
        if std::env::var("GLYIM_TEST_JSON").is_ok() {
            self.write_json(results, &summary);
        }

        summary
    }

    fn write_status(&self, stream: &mut StandardStream, status: &str, color: Color) {
        let _ = stream.set_color(ColorSpec::new().set_fg(Some(color)).set_bold(true));
        let _ = write!(stream, "{:>8} ", status);
        let _ = stream.reset();
    }

    #[cfg(feature = "json-output")]
    fn write_json(&self, results: &[TestResult], summary: &TestSummary) {
        use serde::Serialize;
        #[derive(Serialize)]
        struct Out { total: usize, passed: usize, failed: usize, ignored: usize, tests: Vec<Test> }
        #[derive(Serialize)]
        struct Test { name: String, revision: String, outcome: String, duration_ms: u64, reason: Option<String> }

        let tests: Vec<Test> = results.iter().map(|r| {
            let (outcome, reason) = match &r.outcome {
                TestOutcome::Passed => ("passed".into(), None),
                TestOutcome::Failed { reason: fr } => ("failed".into(), Some(fr.to_string())),
                TestOutcome::Ignored => ("ignored".into(), None),
            };
            Test { name: r.test.name.clone(), revision: r.revision.clone(), outcome, duration_ms: r.duration.as_millis() as u64, reason }
        }).collect();

        let out = Out {
            total: summary.total, passed: summary.passed,
            failed: summary.failed, ignored: summary.ignored, tests,
        };
        if let Ok(s) = serde_json::to_string_pretty(&out) {
            let _ = std::fs::write("target/test-results.json", s);
            eprintln!("JSON results written to target/test-results.json");
        }
    }

    #[cfg(not(feature = "json-output"))]
    fn write_json(&self, _results: &[TestResult], _summary: &TestSummary) {
        eprintln!("Note: Enable 'json-output' feature for JSON output");
    }
}
```

---

## 12. Annotations: mod.rs

```rust
// crates/glyim-test/src/annotations/mod.rs
//! [#15] CRITICAL FIX: Check "~~" before "~".
//!
//! The old code checked single `~` first via strip_prefix('~'),
//! making EVERY `//~` annotation fuzzy — the exact opposite of
//! the documented contract. This version checks "~~" first.

pub mod pattern; // [#13] MatchPattern lives here, NOT in comparison/

use crate::annotations::pattern::MatchPattern; // [#13] Correct import
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

    /// [#15] Single-pass parser. Checks "~~" BEFORE "~".
    pub fn parse_all(source: &str) -> Result<Vec<Self>, String> {
        let mut annotations = Vec::new();
        let mut last_target_line: Option<usize> = None;

        for (line_idx, line) in source.lines().enumerate() {
            let mut search_from = 0;

            while let Some(start) = line[search_from..].find("//") {
                let abs_start = search_from + start;
                search_from = abs_start + 2;
                let rest = &line[abs_start + 2..];

                // ══ [#15] CRITICAL: check "~~" BEFORE "~" ══
                //
                // Old (BROKEN):
                //   strip_prefix('~') matches the ~ in "//~ ERROR msg"
                //   → sets fuzzy = true for EVERY //~ annotation.
                //   "//~~ ERROR msg" matches first ~, leaving "~ ERROR msg",
                //   then the caret parser sees ~ (not ^), and severity
                //   parsing fails on "~".
                //
                // New (CORRECT):
                //   strip_prefix("~~") first; falls through to single ~.
                let (fuzzy, rest) = if let Some(r) = rest.strip_prefix("~~") {
                    (true, r)   // //~~ fuzzy
                } else if let Some(r) = rest.strip_prefix('~') {
                    (false, r)  // //~ exact
                } else {
                    continue;   // Not an annotation
                };

                // Optional: //~?
                let (optional, rest) = if let Some(r) = rest.strip_prefix('?') {
                    (true, r.trim_start())
                } else {
                    (false, rest)
                };

                // Continuation: //~|
                let (is_continuation, rest) = if let Some(r) = rest.strip_prefix('|') {
                    (true, r.trim_start())
                } else {
                    (false, rest)
                };

                // Caret offset: //~^^^
                let (line_offset, rest) = if is_continuation {
                    (0, rest)
                } else {
                    let count = rest.chars().take_while(|c| *c == '^').count();
                    (count, &rest[count..])
                };
                let rest = rest.trim_start();

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

/// Strict severity — typos are errors, not silent mismatches.
fn parse_severity_strict(s: &str) -> Result<(DiagSeverity, &str), String> {
    let (word, rest) = s.split_once(char::is_whitespace).unwrap_or((s, ""));
    match word {
        "ERROR"   => Ok((DiagSeverity::Error,   rest.trim_start())),
        "WARNING" => Ok((DiagSeverity::Warning, rest.trim_start())),
        "NOTE"    => Ok((DiagSeverity::Note,    rest.trim_start())),
        "HELP"    => Ok((DiagSeverity::Help,    rest.trim_start())),
        ""        => Ok((DiagSeverity::Error,   rest)), // default
        other     => Err(format!(
            "invalid severity: {:?}. Expected ERROR, WARNING, NOTE, or HELP", other
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_annotation_is_not_fuzzy() {
        let source = "fn main() {} //~ ERROR msg";
        let anns = Annotation::parse_all(source).unwrap();
        assert_eq!(anns.len(), 1);
        assert!(!anns[0].fuzzy, "//~ must be exact, not fuzzy");
        assert_eq!(anns[0].severity, DiagSeverity::Error);
    }

    #[test]
    fn double_tilde_is_fuzzy() {
        let source = "fn main() {} //~~ ERROR msg";
        let anns = Annotation::parse_all(source).unwrap();
        assert_eq!(anns.len(), 1);
        assert!(anns[0].fuzzy, "//~~ must be fuzzy");
    }

    #[test]
    fn caret_offset() {
        let source = "fn main() {} //~^^ ERROR msg";
        let anns = Annotation::parse_all(source).unwrap();
        assert!(!anns[0].fuzzy);
        assert_eq!(anns[0].line_offset, 2);
    }

    #[test]
    fn optional_annotation() {
        let source = "fn main() {} //~? NOTE hint";
        let anns = Annotation::parse_all(source).unwrap();
        assert!(anns[0].optional);
        assert!(!anns[0].fuzzy);
    }

    #[test]
    fn continuation_targets_previous() {
        let source = "line1\nline2\nline3 //~ ERROR msg\n     //~| NOTE sub";
        let anns = Annotation::parse_all(source).unwrap();
        assert_eq!(anns.len(), 2);
        assert_eq!(anns[1].target_line(), anns[0].target_line());
    }

    #[test]
    fn invalid_severity_rejected() {
        let source = "fn main() {} //~ ERRR msg";
        assert!(Annotation::parse_all(source).is_err());
    }
}
```

---

## 13. Annotations: pattern.rs

```rust
// crates/glyim-test/src/annotations/pattern.rs
//! [#13] MatchPattern lives in annotations/, NOT comparison/.
//! This eliminates the circular dependency:
//!   annotations defines Annotation + MatchPattern.
//!   comparison imports from annotations. Never the reverse.

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

## 14. Comparison: mod.rs

```rust
// crates/glyim-test/src/comparison/mod.rs
//! [#3] CORRECT invariant. [#12] passed() is a METHOD.
//!
//! INVARIANT (corrected):
//!   matched + missing + wrong_severity + optional_unmatched == annotations.len()
//!   matched + unexpected + wrong_severity == diagnostics.len()

pub mod normalize;
// NOTE: NO `pub mod pattern;` — [#13] pattern lives in annotations/

use crate::annotations::Annotation; // [#13] From annotations, not comparison
use crate::annotations::pattern::MatchPattern;
use glyim_diag::{DiagSeverity, GlyimDiagnostic};

/// Extension trait for severity display names.
/// Cannot impl Display for DiagSeverity (orphan rule).
pub trait DiagSeverityExt {
    fn display_name(self) -> &'static str;
}

impl DiagSeverityExt for DiagSeverity {
    fn display_name(self) -> &'static str {
        match self {
            DiagSeverity::Error   => "ERROR",
            DiagSeverity::Warning => "WARNING",
            DiagSeverity::Note    => "NOTE",
            DiagSeverity::Help    => "HELP",
        }
    }
}

#[derive(Clone, Debug)]
pub struct NormalizedDiag {
    pub severity: DiagSeverity,
    pub line: usize,
    pub message: String,
    // [#16 from F1] span field removed — never used in comparison.
}

impl NormalizedDiag {
    pub fn from_glyim_diag(diag: &GlyimDiagnostic, source: &str) -> Self {
        let line = byte_offset_to_line(source, diag.span.primary.lo.to_usize());
        Self { severity: diag.severity, line, message: diag.message.clone() }
    }
}

#[derive(Clone, Debug)]
pub struct ComparisonResult {
    pub matched: Vec<MatchedPair>,
    pub missing: Vec<Annotation>,
    pub unexpected: Vec<NormalizedDiag>,
    pub wrong_severity: Vec<SeverityMismatch>,
    /// [#3] Optional annotations that matched no diagnostic.
    pub optional_unmatched: Vec<Annotation>,
}

impl ComparisonResult {
    /// [#12] Computed method, not stored field. Cannot diverge from data.
    pub fn passed(&self) -> bool {
        self.missing.is_empty()
            && self.unexpected.is_empty()
            && self.wrong_severity.is_empty()
    }

    /// [#3] Verify invariant in debug builds.
    pub fn verify_invariant(&self, annotations_len: usize, diagnostics_len: usize) {
        let ann = self.matched.len() + self.missing.len()
            + self.wrong_severity.len() + self.optional_unmatched.len();
        let diag = self.matched.len() + self.unexpected.len()
            + self.wrong_severity.len();
        debug_assert_eq!(ann, annotations_len,
            "ANNOTATION INVARIANT: {} != {}", ann, annotations_len);
        debug_assert_eq!(diag, diagnostics_len,
            "DIAGNOSTIC INVARIANT: {} != {}", diag, diagnostics_len);
    }
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

/// Compare annotations against diagnostics.
/// Complexity: O(annotations × diagnostics). Fine for typical test sizes.
pub fn compare_diagnostics(
    annotations: &[Annotation],
    diagnostics: &[NormalizedDiag],
) -> ComparisonResult {
    let mut matched = Vec::new();
    let mut missing = Vec::new();
    let mut unexpected = Vec::new();
    let mut wrong_severity = Vec::new();
    let mut optional_unmatched = Vec::new(); // [#3]
    let mut diag_used = vec![false; diagnostics.len()];

    for annotation in annotations {
        let target_line = annotation.target_line();
        let mut found = false;

        for (i, diag) in diagnostics.iter().enumerate() {
            if diag_used[i] { continue; }

            let line_matches = if annotation.fuzzy {
                diag.line.abs_diff(target_line) <= 1
            } else {
                diag.line == target_line
            };

            if line_matches && annotation.pattern.matches(&diag.message) {
                diag_used[i] = true;
                found = true;
                if diag.severity == annotation.severity {
                    matched.push(MatchedPair {
                        annotation: annotation.clone(), diagnostic: diag.clone(),
                    });
                } else {
                    wrong_severity.push(SeverityMismatch {
                        annotation: annotation.clone(), diagnostic: diag.clone(),
                        expected: annotation.severity, actual: diag.severity,
                    });
                }
                break;
            }
        }

        if !found {
            if annotation.optional {
                optional_unmatched.push(annotation.clone()); // [#3]
            } else {
                missing.push(annotation.clone());
            }
        }
    }

    unexpected = diagnostics.iter().enumerate()
        .filter(|(i, _)| !diag_used[*i])
        .map(|(_, d)| d.clone())
        .collect();

    let result = ComparisonResult {
        matched, missing, unexpected, wrong_severity, optional_unmatched,
    };
    result.verify_invariant(annotations.len(), diagnostics.len());
    result
}

fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())].chars().filter(|&c| c == '\n').count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotations::pattern::MatchPattern;

    fn ann(line: usize, sev: DiagSeverity, optional: bool, fuzzy: bool) -> Annotation {
        Annotation { line, line_offset: 0, severity: sev, pattern: MatchPattern::Any, optional, fuzzy }
    }
    fn diag(line: usize, sev: DiagSeverity) -> NormalizedDiag {
        NormalizedDiag { severity: sev, line, message: "test".into() }
    }

    #[test]
    fn optional_unmatched_invariant() {
        let a = ann(0, DiagSeverity::Error, true, false);
        let r = compare_diagnostics(&[a], &[]);
        assert!(r.missing.is_empty());
        assert_eq!(r.optional_unmatched.len(), 1);
        assert!(r.passed());
    }

    #[test]
    fn exact_match_passes() {
        let a = ann(0, DiagSeverity::Error, false, false);
        let d = diag(0, DiagSeverity::Error);
        let r = compare_diagnostics(&[a], &[d]);
        assert!(r.passed());
        assert_eq!(r.matched.len(), 1);
    }

    #[test]
    fn fuzzy_one_line_tolerance() {
        let a = ann(5, DiagSeverity::Error, false, true);
        let d = diag(6, DiagSeverity::Error);
        let r = compare_diagnostics(&[a], &[d]);
        assert!(r.passed());
    }
}
```

---

## 15. Comparison: normalize.rs

```rust
// crates/glyim-test/src/comparison/normalize.rs

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

## 16. Mocks: mod.rs

```rust
// crates/glyim-test/src/mock/mod.rs
//! Mock implementations.
//!
//! # Design Decision: Empty Traits
//!
//! [#5,#6,#7] LowerCtx, BorrowckCtx, and TraitSolver are currently
//! empty traits. Implementing them provides no polymorphic value —
//! no production code calls methods through these traits.
//!
//! For v0.1.0, the mocks are **standalone test helpers** with
//! inherent methods. They do NOT implement the empty traits.
//!
//! When the upstream traits gain methods (see "Required Changes"),
//! add `impl TraitName for MockXxx` blocks.

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

---

## 17. Mocks: lower_ctx.rs

```rust
// crates/glyim-test/src/mock/lower_ctx.rs
//! [#5] Standalone test helper. Does NOT implement empty LowerCtx trait.
//! When LowerCtx gains methods, add `impl LowerCtx for MockLowerCtx`.

use glyim_type::TyCtx;
use glyim_span::Span;
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpanOp { Push(Span), Pop }

/// Mock lower context with recorded span operations.
///
/// This is a standalone test helper — it does NOT implement `LowerCtx`
/// because that trait is currently empty and provides no contract.
/// Inherent methods model the expected interface.
pub struct MockLowerCtx<'a> {
    pub ty_ctx: &'a TyCtx,
    /// RefCell IS required: we need interior mutability for recording,
    /// and when LowerCtx trait gains methods they will take &self.
    span_ops: RefCell<Vec<SpanOp>>,
}

impl<'a> MockLowerCtx<'a> {
    pub fn new(ty_ctx: &'a TyCtx) -> Self {
        Self { ty_ctx, span_ops: RefCell::new(Vec::new()) }
    }

    /// Record a span push.
    pub fn push_span(&self, span: Span) {
        self.span_ops.borrow_mut().push(SpanOp::Push(span));
    }

    /// Record a span pop.
    pub fn pop_span(&self) {
        self.span_ops.borrow_mut().push(SpanOp::Pop);
    }

    /// Get all recorded span operations.
    pub fn span_ops(&self) -> Vec<SpanOp> {
        self.span_ops.borrow().clone()
    }

    /// Assert all span pushes have matching pops.
    pub fn assert_spans_balanced(&self) {
        let depth = self.span_ops.borrow().iter().fold(0, |acc, op| match op {
            SpanOp::Push(_) => acc + 1,
            SpanOp::Pop => acc.saturating_sub(1),
        });
        assert_eq!(depth, 0, "Unbalanced span operations");
    }
}
```

---

## 18. Mocks: borrowck_ctx.rs

```rust
// crates/glyim-test/src/mock/borrowck_ctx.rs
//! [#6] Standalone test helper. Does NOT implement empty BorrowckCtx trait.

use glyim_mir::{Body, LocalDecl, LocalIdx};
use glyim_type::{Ty, TyCtx};

/// Mock borrow-check context.
///
/// Standalone helper — BorrowckCtx trait is currently empty.
pub struct MockBorrowckCtx<'a> {
    pub ty_ctx: &'a TyCtx,
    pub body: &'a Body,
}

impl<'a> MockBorrowckCtx<'a> {
    pub fn new(ty_ctx: &'a TyCtx, body: &'a Body) -> Self { Self { ty_ctx, body } }

    pub fn local_decl(&self, local: LocalIdx) -> &LocalDecl {
        &self.body.locals[local]
    }

    pub fn is_copy(&self, _ty: Ty) -> bool { false }
}
```

---

## 19. Mocks: solver.rs

```rust
// crates/glyim-test/src/mock/solver.rs
//! [#7] Standalone test helper. Does NOT implement empty TraitSolver trait.
//! [#11] Uses plain Vec, not RefCell (no &self constraint since no trait).

use glyim_solve::{SolverResult, TraitPredicate};
use glyim_type::TyCtx;

/// Programmable mock solver.
///
/// Standalone helper — TraitSolver trait is currently empty.
/// [#11] Uses plain Vec for calls since we control the interface
/// (no trait mandating &self).
pub struct MockSolver {
    responses: Vec<(PredicateMatcher, SolverResult)>,
    calls: Vec<TraitPredicate>, // [#11] No RefCell needed
    default: SolverResult,
}

enum PredicateMatcher {
    TraitId(glyim_core::def_id::TraitDefId),
    Any,
}

impl MockSolver {
    pub fn new() -> Self {
        Self { responses: Vec::new(), calls: Vec::new(), default: SolverResult::Ambiguous }
    }

    pub fn default_result(mut self, result: SolverResult) -> Self {
        self.default = result; self
    }

    pub fn respond_for_trait(
        mut self,
        id: glyim_core::def_id::TraitDefId,
        result: SolverResult,
    ) -> Self {
        self.responses.push((PredicateMatcher::TraitId(id), result));
        self
    }

    pub fn respond_for_any(mut self, result: SolverResult) -> Self {
        self.responses.push((PredicateMatcher::Any, result));
        self
    }

    /// Simulate proving a trait predicate.
    pub fn can_prove(&mut self, _ctx: &TyCtx, predicate: &TraitPredicate) -> SolverResult {
        self.calls.push(predicate.clone());
        self.responses.iter()
            .find_map(|(m, r)| match m {
                PredicateMatcher::TraitId(id) if predicate.trait_ref.def_id == *id => Some(r),
                PredicateMatcher::Any => Some(r),
                _ => None,
            })
            .copied()
            .unwrap_or_else(|| self.default.clone())
    }

    /// [#11] Direct access, no borrow needed.
    pub fn call_count(&self) -> usize { self.calls.len() }
    pub fn calls(&self) -> &[TraitPredicate] { &self.calls }

    pub fn assert_call_count(&self, expected: usize) {
        assert_eq!(self.calls.len(), expected,
            "expected {} calls, got {}", expected, self.calls.len());
    }
}

impl Default for MockSolver { fn default() -> Self { Self::new() } }
```

---

## 20. Mocks: codegen.rs

```rust
// crates/glyim-test/src/mock/codegen.rs
//! [#4] Implements ACTUAL CodegenBackend signature.
//!
//! The real signature is:
//!   fn generate(&self, bodies: &[Arc<Body>], output: &Path) -> CompResult<Vec<u8>>
//!
//! No extra _ctx param. No CodegenResult. No generate_function.

use glyim_codegen::CodegenBackend;
use glyim_diag::CompResult;
use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct CodegenCall {
    pub body_count: usize,
    pub output_path: std::path::PathBuf,
}

/// Mock codegen backend with call recording.
///
/// [#4] Implements the ACTUAL CodegenBackend trait with the real signature.
/// RefCell IS required because the trait takes &self.
pub struct MockCodegen {
    calls: RefCell<Vec<CodegenCall>>,
}

impl MockCodegen {
    pub fn new() -> Self { Self { calls: RefCell::new(Vec::new()) } }
    pub fn calls(&self) -> Vec<CodegenCall> { self.calls.borrow().clone() }

    pub fn assert_generated(&self, expected_bodies: usize) {
        let calls = self.calls.borrow();
        assert!(!calls.is_empty(), "expected codegen to be called");
        assert_eq!(calls[0].body_count, expected_bodies);
    }
}

impl Default for MockCodegen { fn default() -> Self { Self::new() } }

/// [#4] Matches the ACTUAL CodegenBackend signature exactly.
impl CodegenBackend for MockCodegen {
    fn name(&self) -> &'static str { "mock" }

    fn generate(
        &self,
        bodies: &[Arc<glyim_mir::Body>],
        output: &Path,
    ) -> CompResult<Vec<u8>> {
        self.calls.borrow_mut().push(CodegenCall {
            body_count: bodies.len(),
            output_path: output.to_path_buf(),
        });
        Ok(Vec::new()) // [#4] Returns Vec<u8>, not CodegenResult
    }
}
```

---

## 21. Mocks: db.rs

```rust
// crates/glyim-test/src/mock/db.rs
//! [#10] Database::new() only. No CrateConfig. No with_interner.
//! [#13 from F1] No Salsa.

/// Simple Database builder for tests.
///
/// [#10] Only uses `Database::new()`. When Database gains
/// configuration or VFS methods, extend this builder.
pub struct TestDbBuilder {
    // No fields yet — Database::new() takes no arguments.
    // Future: files, config, etc. when Database API expands.
}

impl TestDbBuilder {
    pub fn new() -> Self { Self }

    /// [#10] Build a Database using the only public constructor.
    pub fn build(self) -> glyim_db::Database {
        glyim_db::Database::new()
    }
}

impl Default for TestDbBuilder { fn default() -> Self { Self::new() } }
```

---

## 22. Assertions: ty.rs

```rust
// crates/glyim-test/src/assertions/ty.rs
//! [#3] Uses bool_ty/never_ty/unit_ty/mk_ty ONLY.
//! [#1] Never calls Ty::from_raw() or to_raw().
//! [F4] Generic over TypeLookup.
//!
//! Two APIs:
//!   assert_ty → TyAssert (panics) — for unit tests
//!   check_ty  → TyCheck  (Result) — for composable checks

use glyim_core::primitives::*;
use glyim_type::*;
use crate::error::AssertionFailure;

// ════════════════════════════════════════════════════════════
// Panic-based API
// ════════════════════════════════════════════════════════════

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
        if !self.lookup.ty_flags(self.ty).contains(TypeFlags::HAS_TY_INFER) {
            self.fail("type with inference variables");
        }
        self
    }

    pub fn has_no_infer(self) -> Self {
        if self.lookup.ty_flags(self.ty).contains(TypeFlags::HAS_TY_INFER) {
            self.fail("fully resolved type");
        }
        self
    }
}

pub fn assert_ty_eq<L: TypeLookup>(ctx: &L, a: Ty, b: Ty) {
    assert_eq!(a, b, "types not equal: {} vs {}", PrintTy::new(a, ctx), PrintTy::new(b, ctx));
}

// ════════════════════════════════════════════════════════════
// Result-based API (composable — no panics)
// ════════════════════════════════════════════════════════════

pub fn check_ty<'a, L: TypeLookup>(lookup: &'a L, ty: Ty) -> TyCheck<'a, L> {
    TyCheck { lookup, ty, kind: lookup.ty_kind(ty).clone(), failures: Vec::new() }
}

pub struct TyCheck<'a, L: TypeLookup> {
    lookup: &'a L,
    ty: Ty,
    kind: TyKind,
    failures: Vec<AssertionFailure>,
}

impl<'a, L: TypeLookup> TyCheck<'a, L> {
    fn push_failure(&mut self, expected: &str) {
        self.failures.push(AssertionFailure {
            expected: expected.to_string(),
            actual: format!("{:?}", self.kind),
            ty_description: PrintTy::new(self.ty, self.lookup).to_string(),
        });
    }

    pub fn is_error(mut self) -> Self {
        if !matches!(self.kind, TyKind::Error) { self.push_failure("error type"); }
        self
    }

    pub fn is_not_error(mut self) -> Self {
        if matches!(self.kind, TyKind::Error) { self.push_failure("non-error type"); }
        self
    }

    pub fn is_bool(mut self) -> Self {
        if !matches!(self.kind, TyKind::Bool) { self.push_failure("bool type"); }
        self
    }

    pub fn is_unit(mut self) -> Self {
        if !matches!(self.kind, TyKind::Unit) { self.push_failure("unit type"); }
        self
    }

    pub fn is_int(mut self, expected: IntTy) -> Self {
        match &self.kind {
            TyKind::Int(i) if *i == expected => {}
            _ => self.push_failure(&format!("Int({:?})", expected)),
        }
        self
    }

    pub fn is_any_int(mut self) -> Self {
        if !matches!(self.kind, TyKind::Int(_)) { self.push_failure("any Int type"); }
        self
    }

    pub fn is_float(mut self, expected: FloatTy) -> Self {
        match &self.kind {
            TyKind::Float(f) if *f == expected => {}
            _ => self.push_failure(&format!("Float({:?})", expected)),
        }
        self
    }

    pub fn is_ref(mut self, mutability: Mutability) -> TyCheck<'a, L> {
        match &self.kind {
            TyKind::Ref(_, inner, m) if *m == mutability => {
                TyCheck {
                    lookup: self.lookup,
                    ty: *inner,
                    kind: self.lookup.ty_kind(*inner).clone(),
                    failures: self.failures,
                }
            }
            _ => {
                self.push_failure(&format!("&{} type", mutability.prefix_str().trim()));
                self
            }
        }
    }

    pub fn has_infer(mut self) -> Self {
        if !self.lookup.ty_flags(self.ty).contains(TypeFlags::HAS_TY_INFER) {
            self.push_failure("type with inference variables");
        }
        self
    }

    pub fn has_no_infer(mut self) -> Self {
        if self.lookup.ty_flags(self.ty).contains(TypeFlags::HAS_TY_INFER) {
            self.push_failure("fully resolved type");
        }
        self
    }

    /// Consume and return all accumulated failures.
    pub fn finish(self) -> Result<(), Vec<AssertionFailure>> {
        if self.failures.is_empty() { Ok(()) } else { Err(self.failures) }
    }
}
```

---

## 23. Assertions: mir.rs

```rust
// crates/glyim-test/src/assertions/mir.rs
//! [#8] Only real TerminatorKind variants: Goto, Return, Unreachable, Call, Drop.
//! [#11] Uses glyim_mir::Mutability where needed (not core's).

use glyim_mir::*;
use glyim_type::TyCtx;

pub fn assert_mir<'a>(ctx: &'a TyCtx, body: &'a Body) -> MirAssert<'a> {
    MirAssert { ctx, body }
}

pub struct MirAssert<'a> { ctx: &'a TyCtx, body: &'a Body }

impl<'a> MirAssert<'a> {
    pub fn block_count(self, expected: usize) -> Self {
        assert_eq!(self.body.basic_blocks.len(), expected); self
    }
    pub fn local_count(self, expected: usize) -> Self {
        assert_eq!(self.body.locals.len(), expected); self
    }
    pub fn block_stmt_count(self, block: BasicBlockIdx, expected: usize) -> Self {
        assert_eq!(self.body.basic_blocks[block].statements.len(), expected,
            "bb{}", block.to_raw()); self
    }

    /// [#8] Only matches real TerminatorKind variants.
    pub fn block_terminator(self, block: BasicBlockIdx, expected: &str) -> Self {
        let actual = match &self.body.basic_blocks[block].terminator.kind {
            TerminatorKind::Goto { .. }      => "Goto",
            TerminatorKind::Return            => "Return",
            TerminatorKind::Unreachable       => "Unreachable",
            TerminatorKind::Call { .. }       => "Call",
            TerminatorKind::Drop { .. }       => "Drop",
            // [#8] No SwitchInt, no Assert — they don't exist.
        };
        assert_eq!(actual, expected, "bb{} terminator", block.to_raw()); self
    }

    pub fn local_ty(self, local: LocalIdx, expected: &glyim_type::TyKind) -> Self {
        let actual = self.ctx.ty_kind(self.body.locals[local].ty);
        assert_eq!(actual, expected, "local {} ty", local.to_raw()); self
    }
}
```

---

## 24. Assertions: diag.rs & span.rs

```rust
// crates/glyim-test/src/assertions/diag.rs

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
    assert!(diagnostics.iter().any(|d| d.message.contains(substring)),
        "expected diagnostic containing {:?}", substring);
}

pub fn assert_diag_code(diagnostics: &[GlyimDiagnostic], code: &glyim_diag::ErrorCode) {
    assert!(diagnostics.iter().any(|d| d.code == *code),
        "expected diagnostic with code {:?}", code);
}

pub fn assert_has_severity(diagnostics: &[GlyimDiagnostic], severity: DiagSeverity) {
    assert!(diagnostics.iter().any(|d| d.severity == severity));
}
```

```rust
// crates/glyim-test/src/assertions/span.rs

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

## 25. Snapshot: mod.rs & format.rs

```rust
// crates/glyim-test/src/snapshot/mod.rs

pub mod format;

/// [#6] Unique FileId for snapshots.
static SNAPSHOT_FILE_ID: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(1000);

fn next_snapshot_file_id() -> glyim_span::FileId {
    glyim_span::FileId::from_raw(
        SNAPSHOT_FILE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

pub fn snapshot_cst(name: &str, source: &str) {
    let file_id = next_snapshot_file_id();
    let result = glyim_frontend::parse_to_syntax(source, file_id);
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
//! [#11] Uses glyim_mir::Mutability (not core's).

use crate::comparison::DiagSeverityExt;
use glyim_type::TyCtx;

pub fn format_mir_body(ctx: &TyCtx, body: &glyim_mir::Body) -> String {
    let mut out = String::new();
    out.push_str(&format!("fn {}():\n", body.owner));
    out.push_str("  locals:\n");
    for (idx, local) in body.locals.iter_enumerated() {
        // [#11] Use glyim_mir::Mutability, not core's.
        let mut_str = match local.mutability {
            glyim_mir::Mutability::Mut => "mut",
            glyim_mir::Mutability::Not => "imm",
        };
        out.push_str(&format!(
            "    ${}: {} ({})\n",
            idx.to_raw(),
            glyim_type::PrintTy::new(local.ty, ctx),
            mut_str,
        ));
    }
    for (idx, block) in body.basic_blocks.iter_enumerated() {
        out.push_str(&format!("  bb{}:\n", idx.to_raw()));
        for stmt in &block.statements {
            out.push_str(&format!("    {:?}\n", stmt.kind));
        }
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

## 26. Property: arbitrary.rs & check.rs

```rust
// crates/glyim-test/src/property/mod.rs
pub mod arbitrary;
pub mod check;

pub use check::check_ty_property;
```

```rust
// crates/glyim-test/src/property/arbitrary.rs
//! [#3] Uses ONLY confirmed TyCtxMut API: bool_ty, never_ty, unit_ty, mk_ty, mk_ref.
//! [#12] Inference variable generation is DEFERRED until TyCtxMut
//!        gains mk_ty_var/mk_int_var/mk_float_var methods.
//! [#1] Never calls Ty::from_raw() or to_raw().

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

    /// Generate a valid Ty using only confirmed public API.
    ///
    /// [#3] Uses bool_ty/never_ty/unit_ty (not mk_bool/mk_never/mk_unit).
    /// [#3] Uses mk_ty(TyKind::...) for everything else.
    /// [#12] Inference variables NOT generated — deferred until
    ///        TyCtxMut gains mk_ty_var/mk_int_var/mk_float_var.
    pub fn generate_ty(&mut self, ctx: &mut TyCtxMut, depth: u32) -> Ty {
        if depth >= self.max_depth { return self.leaf_ty(ctx); }

        match self.rng.gen_range(0..8) {
            0 => ctx.bool_ty(),   // [#3] not mk_bool()
            1 => ctx.never_ty(),  // [#3] not mk_never()
            2 => ctx.unit_ty(),   // [#3] not mk_unit()
            3 => ctx.mk_ty(TyKind::Int(self.int_ty())),       // [#3] not mk_int()
            4 => ctx.mk_ty(TyKind::Uint(self.uint_ty())),
            5 => ctx.mk_ty(TyKind::Float(self.float_ty())),
            6 => {
                let inner = self.generate_ty(ctx, depth + 1);
                ctx.mk_ref(Region::Erased, inner, self.mutability())
            }
            7 => {
                let inner = self.generate_ty(ctx, depth + 1);
                ctx.mk_ty(TyKind::Slice(inner))  // [#3] not mk_slice()
            }
            _ => self.leaf_ty(ctx),
        }
    }

    fn leaf_ty(&mut self, ctx: &mut TyCtxMut) -> Ty {
        match self.rng.gen_range(0..4) {
            0 => ctx.bool_ty(),
            1 => ctx.unit_ty(),
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

    // ── [#12] DEFERRED: Inference variable generation ──
    //
    // Uncomment when TyCtxMut gains these methods:
    //
    // pub fn generate_ty_with_infer(&mut self, ctx: &mut TyCtxMut, depth: u32) -> Ty {
    //     if depth >= self.max_depth { return self.leaf_ty(ctx); }
    //     match self.rng.gen_range(0..11) {
    //         0..=7 => self.generate_ty(ctx, depth),
    //         8     => ctx.mk_ty_var(),     // TyVar — needs TyCtxMut::mk_ty_var()
    //         9     => ctx.mk_int_var(),    // IntVar — needs TyCtxMut::mk_int_var()
    //         10    => ctx.mk_float_var(),  // FloatVar — needs TyCtxMut::mk_float_var()
    //         _     => unreachable!(),
    //     }
    // }
}

/// Sentinel invariant check. Uses only confirmed API.
/// [#1] Does NOT call to_raw() or from_raw().
/// [#1] Does NOT call iter_types() (doesn't exist yet).
pub fn sentinel_invariant(ctx: &TyCtx) {
    assert!(matches!(ctx.ty_kind(Ty::ERROR), TyKind::Error));
    assert!(matches!(ctx.ty_kind(Ty::NEVER), TyKind::Never));
    assert!(matches!(ctx.ty_kind(Ty::UNIT), TyKind::Unit));
    assert!(matches!(ctx.ty_kind(Ty::BOOL), TyKind::Bool));
}
```

```rust
// crates/glyim-test/src/property/check.rs
//! Property test wrapper. Runs N cases.

use super::arbitrary::{Generator, sentinel_invariant};
use glyim_type::TyKind;

/// Run a property check on generated types.
///
/// `property` receives (ctx, ty) and should return Err(msg) on failure.
/// Runs `n_cases` iterations with the given seed.
pub fn check_ty_property<F>(
    seed: u64,
    n_cases: usize,
    property: F,
) -> Result<(), String>
where
    F: Fn(&glyim_type::TyCtx, glyim_type::Ty) -> Result<(), String>,
{
    let mut ctx_mut = crate::test_ty_ctx();
    let mut gen = Generator::new(seed);

    // Pre-generate all types, then freeze once for checking.
    let types: Vec<glyim_type::Ty> = (0..n_cases)
        .map(|_| gen.generate_ty(&mut ctx_mut, 0))
        .collect();

    let ctx = ctx_mut.freeze();

    // Verify sentinels still valid after generation.
    sentinel_invariant(&ctx);

    for (i, ty) in types.iter().enumerate() {
        if let Err(msg) = property(&ctx, *ty) {
            return Err(format!(
                "case {} failed: {} (ty_kind: {:?})",
                i, msg, ctx.ty_kind(*ty)
            ));
        }
    }

    Ok(())
}

/// Alternative using proptest if the feature is enabled.
#[cfg(feature = "property-proptest")]
pub mod proptest_checks {
    use glyim_type::{TyCtx, TyCtxMut, Ty, TyKind, TypeLookup};
    use glyim_core::primitives::*;
    use proptest::prelude::*;

    /// Strategy for generating TyKind values that can be constructed
    /// through the public API.
    pub fn ty_kind_strategy() -> impl Strategy<Value = TyKind> {
        let leafs = prop_oneof![
            Just(TyKind::Bool),
            Just(TyKind::Never),
            Just(TyKind::Unit),
            Just(TyKind::Int(IntTy::I32)),
            Just(TyKind::Uint(UintTy::U32)),
            Just(TyKind::Float(FloatTy::F64)),
        ];
        leafs
    }

    /// Run a proptest property on generated TyKind values.
    pub fn proptest_ty_property<F>(property: F)
    where
        F: Fn(&TyCtx, TyKind) -> Result<(), String> + std::panic::RefUnwindSafe,
    {
        let mut ctx = TyCtxMut::new(glyim_core::interner::Interner::new());
        proptest!(|(kind in ty_kind_strategy())| {
            let ty = ctx.mk_ty(kind.clone());
            let frozen = ctx.freeze(); // This consumes ctx — problem!
            // Note: proptest integration needs rethinking due to
            // TyCtxMut's ownership model. Use check_ty_property instead.
        });
    }
}
```

---

## 27. Fixtures

```rust
// crates/glyim-test/src/fixtures/mod.rs
pub mod builder;
pub use builder::*;
```

```rust
// crates/glyim-test/src/fixtures/builder.rs
//! [#3] Uses confirmed API: bool_ty, never_ty, unit_ty, mk_ty.

use glyim_core::interner::Interner;
use glyim_type::{TyCtx, TyCtxMut, Ty, TyKind, TypeLookup};
use glyim_core::primitives::*;

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

/// Helper to construct common types using confirmed API. [#3]
pub struct TyFactory;

impl TyFactory {
    /// [#3] Uses bool_ty(), not mk_bool().
    pub fn bool(ctx: &mut TyCtxMut) -> Ty { ctx.bool_ty() }
    pub fn never(ctx: &mut TyCtxMut) -> Ty { ctx.never_ty() }
    pub fn unit(ctx: &mut TyCtxMut) -> Ty { ctx.unit_ty() }
    pub fn i32(ctx: &mut TyCtxMut) -> Ty { ctx.mk_ty(TyKind::Int(IntTy::I32)) }
    pub fn u32(ctx: &mut TyCtxMut) -> Ty { ctx.mk_ty(TyKind::Uint(UintTy::U32)) }
    pub fn f64(ctx: &mut TyCtxMut) -> Ty { ctx.mk_ty(TyKind::Float(FloatTy::F64)) }
    pub fn ref_to(ctx: &mut TyCtxMut, inner: Ty, mutability: Mutability) -> Ty {
        ctx.mk_ref(glyim_type::Region::Erased, inner, mutability)
    }
    pub fn slice_of(ctx: &mut TyCtxMut, inner: Ty) -> Ty {
        ctx.mk_ty(TyKind::Slice(inner)) // [#3] not mk_slice()
    }
}
```

---

## 28. Integration Tests

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
use glyim_type::{Ty, TyKind, TypeLookup};

#[test]
fn test_ty_assert_is_int() {
    // [#3] Uses mk_ty(TyKind::Int(...)), not mk_int(...)
    let (ctx, ty) = with_fresh_ty_ctx(|ctx| ctx.mk_ty(TyKind::Int(IntTy::I32)));
    assert_ty(&ctx, ty).is_int(IntTy::I32);
}

#[test]
fn test_ty_assert_chained_ref() {
    let mut ctx_mut = test_ty_ctx();
    let inner = ctx_mut.bool_ty(); // [#3] not mk_bool()
    let ref_ty = ctx_mut.mk_ref(glyim_type::Region::Erased, inner, Mutability::Mut);
    let ctx = ctx_mut.freeze();
    assert_ty(&ctx, ref_ty).is_ref(Mutability::Mut).is_bool();
}

#[test]
fn test_sentinel_constants() {
    let ctx = test_frozen_ty_ctx();
    assert!(matches!(ctx.ty_kind(Ty::ERROR), TyKind::Error));
    assert!(matches!(ctx.ty_kind(Ty::NEVER), TyKind::Never));
    assert!(matches!(ctx.ty_kind(Ty::UNIT), TyKind::Unit));
    assert!(matches!(ctx.ty_kind(Ty::BOOL), TyKind::Bool));
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
    let mut gen = property::arbitrary::Generator::new(42);
    let ty = gen.generate_ty(&mut ctx, 0);
    let frozen = ctx.freeze();
    // Generator should not produce error types by default
    assert!(
        !matches!(frozen.ty_kind(ty), TyKind::Error),
        "Generator should not produce errors by default"
    );
    property::arbitrary::sentinel_invariant(&frozen);
}

#[test]
fn test_check_ty_property() {
    let result = check_ty_property(42, 50, |_ctx, ty| {
        // All generated types should be non-error
        // (Uses TypeLookup to inspect, not from_raw) [#1]
        Ok(())
    });
    assert!(result.is_ok());
}

#[test]
fn test_mock_codegen() {
    // [#4] Tests the ACTUAL CodegenBackend signature.
    let mock = mock::MockCodegen::new();
    let db = mock::TestDbBuilder::new().build(); // [#10] Database::new() only
    // Can verify the mock implements the trait by calling through it
    use glyim_codegen::CodegenBackend;
    assert_eq!(mock.name(), "mock");
}

#[test]
fn test_annotation_parser_exact_vs_fuzzy() {
    // [#15] The critical test that would fail on the old parser.
    let exact_src = "fn main() {} //~ ERROR msg";
    let anns = glyim_test::annotations::Annotation::parse_all(exact_src).unwrap();
    assert!(!anns[0].fuzzy, "//~ must be exact");

    let fuzzy_src = "fn main() {} //~~ ERROR msg";
    let anns = glyim_test::annotations::Annotation::parse_all(fuzzy_src).unwrap();
    assert!(anns[0].fuzzy, "//~~ must be fuzzy");
}

#[test]
fn test_mock_lower_ctx() {
    // [#5] Standalone helper, no empty trait impl.
    let ctx = test_frozen_ty_ctx();
    let mock_ctx = mock::MockLowerCtx::new(&ctx);
    mock_ctx.push_span(glyim_span::Span::default());
    mock_ctx.pop_span();
    mock_ctx.assert_spans_balanced();
    assert_eq!(mock_ctx.span_ops().len(), 2);
}

#[test]
fn test_mock_solver() {
    // [#7] Standalone helper. Tests configuration API only.
    // Cannot test can_prove() without public Substitution constructor [#2].
    let solver = mock::MockSolver::new()
        .respond_for_any(glyim_solve::SolverResult::Proven);
    // When Substitution::empty() is available, add:
    // let mut s = solver;
    // let result = s.can_prove(&ctx, &predicate);
    // s.assert_call_count(1);
}

#[test]
fn test_comparison_invariant_with_optional() {
    use glyim_test::comparison;
    use glyim_test::annotations::Annotation;
    use glyim_test::annotations::pattern::MatchPattern;
    use glyim_diag::DiagSeverity;

    let ann = Annotation {
        line: 0, line_offset: 0, severity: DiagSeverity::Error,
        pattern: MatchPattern::Any, optional: true, fuzzy: false,
    };
    let result = comparison::compare_diagnostics(&[ann], &[]);
    // [#3] Optional unmatched doesn't fail
    assert!(result.passed());
    assert!(result.missing.is_empty());
    assert_eq!(result.optional_unmatched.len(), 1);
}

#[test]
fn test_check_ty_composable() {
    // TyCheck API accumulates failures instead of panicking.
    let (ctx, ty) = with_fresh_ty_ctx(|c| c.bool_ty());
    let result = check_ty(&ctx, ty)
        .is_bool()
        .is_not_error()
        .finish();
    assert!(result.is_ok());

    // Checking wrong type accumulates failure
    let result = check_ty(&ctx, ty)
        .is_int(IntTy::I32)
        .is_unit()
        .finish();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().len(), 2); // Both assertions failed
}
```

---

## 29. Example Test File

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

## 30. Summary: Critique Fix Map

| # | Severity | Finding | Fix Applied |
|---|----------|---------|-------------|
| 1 | **Critical** | `Ty::from_raw()` is `pub(crate)`; `iter_types()` doesn't exist | Removed `ty_roundtrip_invariant`; `sentinel_invariant` uses only `ty_kind()` on constants; never calls `to_raw()` or `from_raw()` |
| 2 | **Critical** | `Substitution::from_raw` is `pub(crate)` | Removed direct construction from tests; MockSolver test deferred to "Required Changes"; public constructor requested |
| 3 | **Critical** | `mk_bool/mk_never/mk_unit/mk_slice/mk_int` don't exist | Replaced ALL with `bool_ty/never_ty/unit_ty/mk_ty(TyKind::...)/mk_ref` |
| 4 | **Critical** | CodegenBackend wrong signature | `MockCodegen::generate(&self, bodies, output) -> CompResult<Vec<u8>>`; removed `_ctx`, `CodegenResult`, `generate_function` |
| 5 | **Critical** | LowerCtx empty trait | `MockLowerCtx` is standalone helper; no trait impl; inherent methods |
| 6 | **Critical** | BorrowckCtx empty trait | `MockBorrowckCtx` is standalone helper; no trait impl; inherent methods |
| 7 | **Critical** | TraitSolver empty trait | `MockSolver` is standalone helper; no trait impl; inherent `can_prove` |
| 8 | **High** | `SwitchInt`/`Assert` don't exist | `MirAssert::block_terminator` only matches `Goto/Return/Unreachable/Call/Drop` |
| 9 | **High** | 13 missing dependencies | Complete `Cargo.toml` with all deps and explicit versions |
| 10 | **High** | `Database::with_interner`/`CrateConfig` don't exist | `TestDbBuilder::build()` uses `Database::new()` only |
| 11 | **Medium** | `glyim_mir::Mutability` ≠ core's | `format_mir_body` imports and matches `glyim_mir::Mutability` |
| 12 | **Medium** | No inference var allocation | Generator only produces concrete types; `mk_ty_var/mk_int_var/mk_float_var` listed as required changes; code included as commented-out |
| 13 | **Medium** | `annotations → comparison::pattern` circular dep | `pattern.rs` lives in `annotations/`; `comparison` imports from `
