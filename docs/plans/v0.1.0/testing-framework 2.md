# `glyim-test` — Complete Redesigned Plan

The critical fix: the test executor now drives the **actual compilation pipeline**, collecting diagnostics from every phase. Compile-fail tests for type errors, borrow errors, and name resolution errors all work correctly. No stub compilation.

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
glyim-opt        = { workspace = true }
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
```

---

## Module Structure

```
src/
├── lib.rs
├── harness/
│   ├── mod.rs
│   ├── config.rs
│   ├── collector.rs
│   ├── plan.rs
│   ├── executor.rs       # Drives ACTUAL pipeline
│   ├── compiler.rs       # Per-phase diagnostic collection
│   └── reporter.rs
├── annotations/
│   ├── mod.rs
│   └── pattern.rs
├── comparison/
│   ├── mod.rs
│   └── normalize.rs
├── mock/
│   ├── mod.rs
│   ├── lower_ctx.rs
│   ├── borrowck_ctx.rs
│   ├── solver.rs
│   ├── codegen.rs
│   └── db.rs
├── assertions/
│   ├── mod.rs
│   ├── ty.rs
│   ├── mir.rs
│   ├── diag.rs
│   └── span.rs
├── snapshot/
│   ├── mod.rs
│   └── format.rs
├── property/
│   ├── mod.rs
│   └── arbitrary.rs
└── fixtures/
    ├── mod.rs
    └── builder.rs
```

---

## `lib.rs`

```rust
// crates/glyim-test/src/lib.rs
//! State-of-the-art compiler testing framework for Glyim.
//!
//! # Critical Design: Real Pipeline Execution
//!
//! The test executor drives the **actual compilation pipeline**,
//! collecting diagnostics from every phase (parse → def-map → hir →
//! typeck → mir → borrowck). Compile-fail tests for type errors,
//! borrow errors, and name resolution all work correctly because
//! the real compiler runs, not a stub.
//!
//! # Infrastructure Leverage
//!
//! - **[F1]** Never calls `Ty::from_raw()`. Uses `Ty::ERROR` sentinels.
//! - **[F2]** All sentinels are `pub const` on `Ty`.
//! - **[F4]** `TypeLookup` trait used throughout for display/inspection.
//! - **[F13]** No Salsa. `TestDbBuilder` creates a simple `Database`.
//! - **[F16]** `DiagSink::new()` provides default logging.
//! - **[F18]** Separate `IntVar`, `FloatVar`, `TyVar` respected.
//!
//! # File Extension
//!
//! Test files use the `.g` extension (NOT `.gly`).
//!
//! # Error Annotations
//!
//! ```gly
//! //~ ERROR message      — exact line match
//! //~^^ ERROR message    — N lines above
//! //~| NOTE message      — same target as previous
//! //~? ERROR message     — optional (won't fail if unmatched)
//! //~~ ERROR message     — fuzzy (1-line tolerance)
//! ```
//!
//! # Environment Variables
//!
//! - `GLYIM_BLESS=1` — auto-update expected output files
//! - `GLYIM_TEST_SHOW_OUTPUT=1` — verbose stderr output

pub mod harness;
pub mod annotations;
pub mod comparison;
pub mod mock;
pub mod assertions;
pub mod snapshot;
pub mod property;
pub mod fixtures;

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
use glyim_type::{TyCtx, TyCtxMut, Ty, TypeLookup};

/// Create a `TyCtxMut` for testing.
pub fn test_ty_ctx() -> TyCtxMut { TyCtxBuilder::new().build_mut() }

/// Create a frozen `TyCtx` for testing.
pub fn test_frozen_ty_ctx() -> TyCtx { test_ty_ctx().freeze() }

/// Run a function with a fresh `TyCtxMut` and freeze the result.
pub fn with_fresh_ty_ctx<F, R>(f: F) -> (TyCtx, R)
where F: FnOnce(&mut TyCtxMut) -> R,
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
//! Strict parsing. `.g` extension. Hyphens only.

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TestMode {
    CompilePass,
    CompileFail,
    Ui,
}

impl TestMode {
    pub fn from_str_exact(s: &str) -> Result<Self, String> {
        match s {
            "compile-pass" => Ok(Self::CompilePass),
            "compile-fail" => Ok(Self::CompileFail),
            "ui" => Ok(Self::Ui),
            _ => Err(format!("unknown test-mode: {:?}. Expected: compile-pass, compile-fail, ui", s)),
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

pub fn parse_test_config(source: &str) -> Result<TestConfig, String> {
    let mut config = TestConfig::default();

    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("//") {
            if trimmed.is_empty() { continue; }
            break;
        }
        let content = trimmed[2..].trim();

        if let Some(rest) = content.strip_prefix('[') {
            if let Some(end) = rest.find(']') {
                let rev = &rest[..end];
                let directive = rest[end + 1..].trim();
                if let Some(value) = directive.strip_prefix("compile-flags:") {
                    config.revision_compile_flags
                        .entry(rev.to_string())
                        .or_default()
                        .extend(shell_words::split(value.trim()).map_err(|e| format!("invalid flags: {}", e))?);
                }
                continue;
            }
        }

        if let Some(value) = content.strip_prefix("test-mode:") {
            config.mode = TestMode::from_str_exact(value.trim())?;
        } else if let Some(value) = content.strip_prefix("revisions:") {
            config.revisions = value.split_whitespace().map(String::from).collect();
        } else if let Some(value) = content.strip_prefix("compile-flags:") {
            config.compile_flags.extend(shell_words::split(value.trim()).map_err(|e| format!("invalid flags: {}", e))?);
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
//! Discovers `.g` files. Fails loudly if root missing.

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

pub struct TestCollector<'a> { root: &'a Path }

impl<'a> TestCollector<'a> {
    pub fn new(root: &'a Path) -> Self { Self { root } }

    pub fn collect(
        &self,
        filter: Option<&str>,
        mode_override: Option<TestMode>,
    ) -> Result<Vec<DiscoveredTest>, String> {
        if !self.root.exists() {
            return Err(format!("test directory does not exist: {:?}", self.root));
        }

        let mut tests = Vec::new();

        for entry in WalkDir::self.root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("g") { continue; }
            if let Some(f) = filter {
                if !path.to_string_lossy().contains(f) { continue; }
            }

            let source: Arc<str>
