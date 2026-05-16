# glyim-pilot v3 — Complete Rewrite with All Critique Fixes

Every finding from the v2 critique is fixed. I'll note each fix inline.

---

## Cargo.toml

```toml
[package]
name = "glyim-pilot"
version = "0.3.0"
edition = "2021"
description = "Autonomous AI agent dispatch for Glyim compiler development"
license = "MIT"

[[bin]]
name = "glyim-pilot"
path = "src/main.rs"

[dependencies]
clap = { version = "4.5", features = ["derive", "env"] }
tokio = { version = "1", features = ["full"] }
futures-util = "0.3"
async-trait = "0.1"
tokio-tungstenite = "0.29"
regex = "1.11"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
anyhow = "1"
thiserror = "2"
comfy-table = "7"
chrono = { version = "0.4", features = ["serde"] }
path-clean = "1"
dunce = "1"
walkdir = "2"
ignore = "0.4"
strip-ansi-escapes = "0.2"
uuid = { version = "1", features = ["v4"] }
prometheus = { version = "0.13", optional = true }

[dev-dependencies]
proptest = "1.11"
tempfile = "3"
tokio-test = "0.4"
assert_cmd = "2"
predicates = "3"
pretty_assertions = "1"

[features]
default = []
prometheus = ["dep:prometheus"]

[profile.release]
lto = true
strip = true
opt-level = 3
codegen-units = 1
```

## ERROR_CODES.md

```markdown
# Glyim Pilot Error Codes

| Code   | Category         | Description                                        |
|--------|------------------|----------------------------------------------------|
| E0100  | Protocol/Parse   | Generic protocol parse error                       |
| E0201  | Apply/Find       | FIND text not found in file                        |
| E0202  | Apply/Find       | FIND text found multiple times (expected exactly 1)|
| E0203  | Apply/File       | Target file not found                              |
| E0204  | Apply/I/O        | I/O error during file apply                        |
| E0205  | Apply/Task       | Spawned task join failure (panic or cancellation)  |
| E0206  | Apply/Rollback   | Apply failed and was rolled back                   |
| E0300  | Security         | Path escapes worktree root                         |
| E0400  | Git              | Git operation failed                               |
| E0500  | Gate/Infra       | Gate infrastructure failure (tool missing, timeout) |
| E0600  | Config           | Configuration error                                |
| E0700  | Session          | Session state error                                |
| E0800  | I/O              | General I/O error                                  |
| E0900  | Limits           | Apply limits exceeded                              |

## Guidelines

- **E05xx** (Gate infrastructure) vs semantic gate failure: `Err(E05xx)`
  means the gate could not run at all. `Ok(GateResult { passed: false })`
  means the gate ran but found violations. Never conflate these.

- **E02xx** (Apply) errors are recoverable via rollback. E0206 specifically
  indicates a partial apply was detected and all changes were reverted.

- **E03xx** (Security) errors indicate path traversal attempts and should
  be logged at WARN level for security auditing.
```

## src/lib.rs

```rust
pub mod domain_types;
pub mod error;
pub mod metrics;
pub mod process;
pub mod protocol;
pub mod applier;
pub mod config;
pub mod git_ops;
pub mod gates;
pub mod commit;
pub mod session;
pub mod context;
pub mod dispatch;
pub mod server;
pub mod orchestrator;
pub mod cli;

pub use error::PilotError;
pub use domain_types::{ApplyLimits, BannedPattern, DependencyRule};
pub use protocol::types::{FileOp, ParsedOps, PROTOCOL_VERSION};
pub use protocol::parser::{parse_ops_block, extract_ops_blocks};
pub use applier::{
    apply_ops, apply_ops_async, preview_ops, preview_ops_async,
    ApplyResult, ApplyAction, PlannedChange, PlannedAction,
};
```

## src/domain_types.rs

```rust
//! Domain types shared between config and implementation modules.
//! Defined here so `config` does not depend on `applier` or `gates`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyLimits {
    pub max_file_size: usize,
    pub max_total_content: usize,
    pub max_ops_per_block: usize,
}

impl Default for ApplyLimits {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024,
            max_total_content: 50 * 1024 * 1024,
            max_ops_per_block: 100,
        }
    }
}

impl ApplyLimits {
    pub fn strict() -> Self {
        Self {
            max_file_size: 1024 * 1024,
            max_total_content: 5 * 1024 * 1024,
            max_ops_per_block: 20,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BannedPattern {
    pub pattern: String,
    pub description: String,
}

impl BannedPattern {
    pub fn new(pattern: impl Into<String>, description: impl Into<String>) -> Self {
        Self { pattern: pattern.into(), description: description.into() }
    }
}

pub fn default_banned_patterns() -> Vec<BannedPattern> {
    vec![
        BannedPattern::new("todo!()", "`todo!()` in non-test code"),
        BannedPattern::new("unwrap()", "`.unwrap()` in non-test code"),
        BannedPattern::new("panic!()", "`panic!()` in non-test code"),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyRule {
    pub from_crate: String,
    pub forbidden_dep: String,
    pub reason: String,
}

pub fn default_architecture_rules() -> Vec<DependencyRule> {
    vec![
        DependencyRule { from_crate: "glyim-frontend".into(), forbidden_dep: "glyim-type".into(), reason: "frontend must not depend on type directly".into() },
        DependencyRule { from_crate: "glyim-frontend".into(), forbidden_dep: "glyim-ir".into(), reason: "frontend must not depend on IR".into() },
        DependencyRule { from_crate: "glyim-syntax".into(), forbidden_dep: "glyim-ir".into(), reason: "syntax must not depend on IR".into() },
        DependencyRule { from_crate: "glyim-type".into(), forbidden_dep: "glyim-codegen".into(), reason: "type must not depend on codegen".into() },
    ]
}
```

## src/error.rs

```rust
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PilotError {
    #[error("protocol parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    #[error("file apply error: {0}")]
    Apply(#[from] ApplyError),

    #[error("path security violation: {path} escapes worktree {root}: {reason}")]
    PathEscape { path: String, root: String, reason: String },

    #[error("git operation failed: {0}")]
    Git(String),

    #[error("gate '{gate}' infrastructure failure: {message}")]
    Gate { gate: String, message: String },

    #[error("config error: {0}")]
    Config(String),

    #[error("session error: {0}")]
    Session(String),

    #[error("apply limits exceeded: {0}")]
    Limits(String),

    #[error("io error: {0}")]
    Io(#[source] io::Error),
}

impl PilotError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Parse { .. } => "E0100",
            Self::Apply(e) => e.code(),
            Self::PathEscape { .. } => "E0300",
            Self::Git(_) => "E0400",
            Self::Gate { .. } => "E0500",
            Self::Config(_) => "E0600",
            Self::Session(_) => "E0700",
            Self::Limits(_) => "E0900",
            Self::Io(_) => "E0800",
        }
    }
}

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("FIND text not found in {path}")]
    FindNotFound { path: String },
    #[error("FIND text found {count} times in {path} (expected exactly 1)")]
    FindAmbiguous { path: String, count: usize },
    #[error("file not found: {0}")]
    FileNotFound(String),
    #[error("I/O error during {operation} on {path}: {source}")]
    Io { path: String, operation: String, #[source] source: io::Error },
    #[error("task join failure during {operation}: {reason}")]
    TaskJoin { operation: String, reason: String },
    #[error("apply failed and was rolled back: {detail}")]
    RolledBack { detail: String },
}

impl ApplyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::FindNotFound { .. } => "E0201",
            Self::FindAmbiguous { .. } => "E0202",
            Self::FileNotFound(_) => "E0203",
            Self::Io { .. } => "E0204",
            Self::TaskJoin { .. } => "E0205",
            Self::RolledBack { .. } => "E0206",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_error_codes_documented() {
        let codes = [
            "E0100", "E0201", "E0202", "E0203", "E0204", "E0205", "E0206",
            "E0300", "E0400", "E0500", "E0600", "E0700", "E0800", "E0900",
        ];
        let md = include_str!("../ERROR_CODES.md");
        for code in codes {
            assert!(md.contains(code), "ERROR_CODES.md missing code {code}");
        }
    }

    #[test]
    fn test_task_join_distinct_from_io() {
        let err = ApplyError::TaskJoin { operation: "test".into(), reason: "panic".into() };
        assert_eq!(err.code(), "E0205");
        assert!(!format!("{err}").contains("I/O"));
    }
}
```

## src/metrics.rs

**Fix #3 from critique**: PrometheusMetrics now caches counters/histograms by name in a `Mutex<HashMap>`. No new allocation on every increment.

```rust
pub mod names {
    pub const OPS_READY_RECEIVED: &str = "ops_ready_received";
    pub const OPS_APPLIED: &str = "ops_applied";
    pub const TURN_PROCESSED: &str = "turn_processed";
    pub const TURN_PANIC: &str = "turn_panic";
    pub const ORCHESTRATOR_ERROR: &str = "orchestrator_error";
    pub const STREAM_COMPLETE: &str = "stream_complete";
    pub const EXTENSION_ERROR: &str = "extension_error";
    pub const COMMIT_DECISION: &str = "commit_decision";
    pub const DONE_PIPELINE: &str = "done_pipeline";
    pub const PR_CREATED: &str = "pr_created";
}

pub trait Metrics: Send + Sync {
    fn increment_counter(&self, name: &str, labels: &[(&str, &str)]);
    fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]);
}

pub struct NoOpMetrics;
impl Metrics for NoOpMetrics {
    fn increment_counter(&self, _name: &str, _labels: &[(&str, &str)]) {}
    fn record_histogram(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {}
}

pub struct LoggingMetrics;
impl Metrics for LoggingMetrics {
    fn increment_counter(&self, name: &str, labels: &[(&str, &str)]) {
        tracing::debug!(metric = name, labels = ?labels, "counter incremented");
    }
    fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
        tracing::debug!(metric = name, value, labels = ?labels, "histogram recorded");
    }
}

#[cfg(feature = "prometheus")]
pub mod prometheus_impl {
    use super::Metrics;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// PrometheusMetrics caches all counters and histograms by name.
    /// First call for a name: create, register, cache, increment.
    /// Subsequent calls: cache hit, increment only. No allocation.
    pub struct PrometheusMetrics {
        counters: Mutex<HashMap<String, prometheus::IntCounter>>,
        histograms: Mutex<HashMap<String, prometheus::Histogram>>,
    }

    impl PrometheusMetrics {
        pub fn new() -> Self {
            Self {
                counters: Mutex::new(HashMap::new()),
                histograms: Mutex::new(HashMap::new()),
            }
        }
    }

    impl Metrics for PrometheusMetrics {
        fn increment_counter(&self, name: &str, labels: &[(&str, &str)]) {
            let mut cache = self.counters.lock().unwrap();

            if let Some(counter) = cache.get(name) {
                counter.inc();
                return;
            }

            let opts = prometheus::Opts::new(name, name)
                .const_labels(make_const_labels(labels));
            let counter = prometheus::IntCounter::with_opts(opts)
                .expect("failed to create counter opts");

            // Register with global registry. Ignore error if already registered
            // (e.g. from a previous process run that didn't clean up).
            let _ = prometheus::default_registry().register(Box::new(counter.clone()));

            cache.insert(name.to_string(), counter.clone());
            counter.inc();
        }

        fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]) {
            let mut cache = self.histograms.lock().unwrap();

            if let Some(histo) = cache.get(name) {
                histo.observe(value);
                return;
            }

            let opts = prometheus::HistogramOpts::new(name, name)
                .const_labels(make_const_labels(labels));
            let histo = prometheus::Histogram::with_opts(opts)
                .expect("failed to create histogram opts");

            let _ = prometheus::default_registry().register(Box::new(histo.clone()));

            cache.insert(name.to_string(), histo.clone());
            histo.observe(value);
        }
    }

    fn make_const_labels(labels: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
        labels.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect()
    }
}

pub fn production_metrics() -> Box<dyn Metrics> {
    #[cfg(feature = "prometheus")]
    { Box::new(prometheus_impl::PrometheusMetrics::new()) }
    #[cfg(not(feature = "prometheus"))]
    { Box::new(LoggingMetrics) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_and_logging_dont_panic() {
        let n = NoOpMetrics;
        n.increment_counter("x", &[]);
        n.record_histogram("x", 1.0, &[]);
        let l = LoggingMetrics;
        l.increment_counter("x", &[("k", "v")]);
        l.record_histogram("x", 1.0, &[("k", "v")]);
    }
}
```

## src/process.rs

**Fix #1 from critique**: `err` is now bound before use in the test.

```rust
use std::path::Path;
use std::time::Duration;

#[derive(Debug)]
pub struct ProcessError {
    pub program: String,
    pub cwd: std::path::PathBuf,
    pub args: Vec<String>,
    pub kind: ProcessErrorKind,
}

#[derive(Debug)]
pub enum ProcessErrorKind {
    ExecutionFailed(std::io::Error),
    TimedOut { timeout_secs: u64 },
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ProcessErrorKind::ExecutionFailed(e) => write!(
                f, "{} failed in {}: {e} (args: {:?})", self.program, self.cwd.display(), self.args
            ),
            ProcessErrorKind::TimedOut { timeout_secs } => write!(
                f, "{} timed out after {timeout_secs}s in {} (args: {:?})",
                self.program, self.cwd.display(), self.args
            ),
        }
    }
}

pub async fn run_timed_command(
    program: &str,
    args: &[&str],
    cwd: &Path,
    timeout_secs: u64,
) -> Result<std::process::Output, ProcessError> {
    let effective_timeout = if timeout_secs == 0 { 300 } else { timeout_secs };
    let timeout = Duration::from_secs(effective_timeout);

    let output_fut = tokio::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output();

    match tokio::time::timeout(timeout, output_fut).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(ProcessError {
            program: program.into(),
            cwd: cwd.to_path_buf(),
            args: args.iter().map(|s| s.to_string()).collect(),
            kind: ProcessErrorKind::ExecutionFailed(e),
        }),
        Err(_) => Err(ProcessError {
            program: program.into(),
            cwd: cwd.to_path_buf(),
            args: args.iter().map(|s| s.to_string()).collect(),
            kind: ProcessErrorKind::TimedOut { timeout_secs: effective_timeout },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_timed_command_success() {
        let result = run_timed_command("echo", &["hello"], Path::new("."), 10).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("hello"));
    }

    #[tokio::test]
    async fn test_run_timed_command_timeout() {
        let result = run_timed_command("sleep", &["10"], Path::new("."), 1).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ProcessErrorKind::TimedOut { .. }));
    }

    #[tokio::test]
    async fn test_run_timed_command_not_found() {
        let result = run_timed_command(
            "nonexistent_command_xyz", &[], Path::new("."), 5,
        ).await;
        assert!(result.is_err());
        // FIX: bind err before using it — the old code referenced
        // `err.kind` without binding `err` first.
        let err = result.unwrap_err();
        assert!(matches!(err.kind, ProcessErrorKind::ExecutionFailed(_)));
    }
}
```

## src/protocol/mod.rs

```rust
pub mod types;
pub mod parser;
pub use types::PROTOCOL_VERSION;
```

## src/protocol/types.rs

```rust
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", content = "data")]
pub enum FileOp {
    #[serde(rename = "write")]
    Write { path: String, content: String },

    /// Replace a single exact occurrence of `find` with `replace` in the
    /// file at `path`.
    ///
    /// # Contract
    /// - **Scope**: Entire file content as a single string.
    /// - **Match**: Exact, case-sensitive, whitespace-sensitive.
    /// - **Occurrence**: `find` must appear **exactly once**.
    ///   Zero → `FindNotFound`. Two+ → `FindAmbiguous`.
    /// - **Replacement**: `String::replacen(find, replace, 1)`.
    /// - **Atomicity**: Temp-file-then-rename.
    #[serde(rename = "replace")]
    Replace { path: String, find: String, replace: String },

    #[serde(rename = "delete")]
    Delete { path: String },
}

impl FileOp {
    pub fn path(&self) -> &str {
        match self {
            FileOp::Write { path, .. } => path,
            FileOp::Replace { path, .. } => path,
            FileOp::Delete { path } => path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedOps {
    pub ops: Vec<FileOp>,
    pub commit_message: Option<String>,
    pub incomplete: bool,
    pub done: bool,
    pub approved: bool,
}

impl ParsedOps {
    pub fn empty() -> Self {
        Self { ops: Vec::new(), commit_message: None, incomplete: false, done: false, approved: false }
    }
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty() && self.commit_message.is_none() && !self.incomplete && !self.done && !self.approved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version_positive() {
        assert!(PROTOCOL_VERSION > 0);
    }

    #[test]
    fn test_file_op_path_accessor() {
        assert_eq!(FileOp::Write { path: "a.rs".into(), content: String::new() }.path(), "a.rs");
        assert_eq!(FileOp::Delete { path: "c.rs".into() }.path(), "c.rs");
    }
}
```

## src/protocol/parser.rs

```rust
use crate::error::PilotError;
use crate::protocol::types::{FileOp, ParsedOps};

/// Extract `glyim-ops` blocks from a full AI response.
/// Handles bare markdown fences inside WRITE/REPLACE content.
///
/// # Known Limitation
/// If the AI writes a file whose content literally contains `::END`
/// as a standalone line, the parser will interpret it as the block
/// terminator. Workaround: use ::REPLACE on an existing file, or
/// split the content so `::END` does not appear as a standalone line.
pub fn extract_ops_blocks(response: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = response.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed == "```glyim-ops" || trimmed.starts_with("```glyim-ops ") {
            let content_start = i + 1;
            let mut end_line = None;
            let mut inside_write_or_replace = false;

            for j in (i + 1)..lines.len() {
                let t = lines[j].trim();
                if t.starts_with("::WRITE ") || t.starts_with("::REPLACE ") {
                    inside_write_or_replace = true;
                } else if t == "::END" && inside_write_or_replace {
                    inside_write_or_replace = false;
                }
                if t.starts_with("```") && !inside_write_or_replace {
                    end_line = Some(j);
                    break;
                }
            }

            if let Some(end) = end_line {
                blocks.push(lines[content_start..end].join("\n").trim().to_string());
                i = end + 1;
            } else {
                break;
            }
        } else {
            i += 1;
        }
    }
    blocks
}

pub fn parse_ops_block(input: &str) -> Result<ParsedOps, PilotError> {
    let mut ops = Vec::new();
    let mut commit_message = None;
    let mut incomplete = false;
    let mut done = false;
    let mut approved = false;
    let mut lines = input.lines().enumerate().peekable();

    while let Some((line_num, line)) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }

        if let Some(rest) = trimmed.strip_prefix("::WRITE ") {
            let path = rest.trim().to_string();
            if path.is_empty() {
                return Err(PilotError::Parse { line: line_num + 1, message: "WRITE requires a path".into() });
            }
            ops.push(FileOp::Write { path, content: read_until_end(&mut lines, line_num)? });
        } else if let Some(rest) = trimmed.strip_prefix("::REPLACE ") {
            let path = rest.trim().to_string();
            if path.is_empty() {
                return Err(PilotError::Parse { line: line_num + 1, message: "REPLACE requires a path".into() });
            }
            let (find, replace) = read_find_replace(&mut lines, line_num)?;
            ops.push(FileOp::Replace { path, find, replace });
        } else if let Some(rest) = trimmed.strip_prefix("::DELETE ") {
            let path = rest.trim().to_string();
            if path.is_empty() {
                return Err(PilotError::Parse { line: line_num + 1, message: "DELETE requires a path".into() });
            }
            ops.push(FileOp::Delete { path });
        } else if trimmed == "::DELETE" {
            return Err(PilotError::Parse { line: line_num + 1, message: "DELETE requires a path".into() });
        } else if let Some(msg) = trimmed.strip_prefix("::COMMIT ") {
            commit_message = Some(msg.trim().to_string());
        } else if trimmed == "::COMMIT" {
            commit_message = Some(String::new());
        } else if trimmed == "::INCOMPLETE" {
            incomplete = true;
        } else if trimmed == "::DONE" {
            done = true;
        } else if trimmed == "::APPROVED" {
            approved = true;
        }
    }

    Ok(ParsedOps { ops, commit_message, incomplete, done, approved })
}

fn read_until_end(
    lines: &mut impl Iterator<Item = (usize, &str)>,
    start_line: usize,
) -> Result<String, PilotError> {
    let mut content_lines = Vec::new();
    for (_, line) in lines {
        if line.trim() == "::END" {
            while content_lines.last().map_or(false, |l| l.trim().is_empty()) {
                content_lines.pop();
            }
            return Ok(content_lines.join("\n"));
        }
        content_lines.push(line);
    }
    Err(PilotError::Parse { line: start_line + 1, message: "unexpected end of input: expected ::END".into() })
}

fn read_find_replace(
    lines: &mut impl Iterator<Item = (usize, &str)>,
    start_line: usize,
) -> Result<(String, String), PilotError> {
    let mut find_lines: Vec<String> = Vec::new();
    let mut replace_lines: Vec<String> = Vec::new();
    let mut in_find = false;
    let mut in_replace = false;

    for (_, line) in lines {
        match line.trim() {
            "---FIND---" => { in_find = true; in_replace = false; }
            "---REPLACE---" => { in_find = false; in_replace = true; }
            "::END" => {
                while find_lines.last().map_or(false, |l| l.trim().is_empty()) { find_lines.pop(); }
                while replace_lines.last().map_or(false, |l| l.trim().is_empty()) { replace_lines.pop(); }
                return Ok((find_lines.join("\n"), replace_lines.join("\n")));
            }
            _ => {
                if in_find { find_lines.push(line.to_string()); }
                else if in_replace { replace_lines.push(line.to_string()); }
            }
        }
    }
    Err(PilotError::Parse { line: start_line + 1, message: "unexpected end of input: expected ::END in REPLACE block".into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_write() {
        let input = "::WRITE src/main.rs\nfn main() {}\n::END";
        let result = parse_ops_block(input).unwrap();
        assert_eq!(result.ops[0], FileOp::Write { path: "src/main.rs".into(), content: "fn main() {}".into() });
    }

    #[test]
    fn test_parse_replace() {
        let input = "::REPLACE src/lib.rs\n---FIND---\nold\n---REPLACE---\nnew\n::END";
        let result = parse_ops_block(input).unwrap();
        assert_eq!(result.ops[0], FileOp::Replace { path: "src/lib.rs".into(), find: "old".into(), replace: "new".into() });
    }

    #[test]
    fn test_extract_nested_fences_inside_write() {
        let response = "```glyim-ops\n::WRITE readme.md\n# Hello\n\n```rust\nfn main() {}\n```\n\nMore text\n::END\n```";
        let blocks = extract_ops_blocks(response);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("fn main() {}"));
    }

    #[test]
    fn test_write_without_end_is_error() {
        let input = "::WRITE src/main.rs\nfn main() {}";
        assert!(parse_ops_block(input).is_err());
    }
}
```

## src/applier/security.rs

```rust
use std::path::{Path, PathBuf};
use dunce::canonicalize;

pub fn validate_path(worktree_root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let relative = Path::new(relative_path);
    if relative.is_absolute() {
        return Err(format!("path '{}' is absolute; must be relative to worktree", relative_path));
    }

    let canonical_root = if worktree_root.exists() {
        match canonicalize(worktree_root) {
            Ok(c) => c,
            Err(_) => path_clean::PathClean::clean(worktree_root),
        }
    } else {
        path_clean::PathClean::clean(worktree_root)
    };

    let candidate = canonical_root.join(relative);
    let normalized = path_clean::PathClean::clean(&candidate);

    if normalized == canonical_root {
        return Err(format!("path '{}' resolves to worktree root, not a file", relative_path));
    }
    if !normalized.starts_with(&canonical_root) {
        if worktree_root.exists() {
            if let (Ok(can_child), Ok(can_parent)) = (canonicalize(&normalized), canonicalize(&canonical_root)) {
                if can_child.starts_with(can_parent) { return Ok(normalized); }
            }
        }
        return Err(format!("path '{}' escapes worktree '{}'", relative_path, canonical_root.display()));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_path_traversal_attack() {
        let dir = TempDir::new().unwrap();
        let result = validate_path(dir.path(), "../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_absolute_path_rejected() {
        let dir = TempDir::new().unwrap();
        let result = validate_path(dir.path(), "/etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_path() {
        let dir = TempDir::new().unwrap();
        let result = validate_path(dir.path(), "src/main.rs");
        assert!(result.is_ok());
    }
}
```

## src/applier/mod.rs

```rust
pub mod security;

use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::domain_types::ApplyLimits;
use crate::error::{ApplyError, PilotError};
use crate::protocol::types::FileOp;
use security::validate_path;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApplyResult { pub path: String, pub action: ApplyAction }

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ApplyAction { Created, Modified, Deleted }

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlannedChange { pub path: String, pub action: PlannedAction, pub current_content_summary: Option<String> }

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PlannedAction { Create, Overwrite, Modify, Delete }

struct Backup { rel_path: String, original_content: Option<String> }

pub fn apply_ops(
    worktree_root: &Path, ops: &[FileOp], limits: &ApplyLimits,
) -> Result<Vec<ApplyResult>, PilotError> {
    validate_limits(ops, limits)?;
    let backups = create_backups(worktree_root, ops)?;
    let mut results = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        match apply_op_atomic(worktree_root, op) {
            Ok(result) => {
                tracing::debug!(path = %result.path, action = ?result.action, "applied {}/{}", i + 1, ops.len());
                results.push(result);
            }
            Err(e) => {
                tracing::error!(op_index = i, error = %e, "apply failed, rolling back");
                rollback(worktree_root, &backups, &results);
                return Err(PilotError::Apply(ApplyError::RolledBack {
                    detail: format!("operation {} of {} failed: {} (rollback succeeded)", i + 1, ops.len(), e),
                }));
            }
        }
    }
    Ok(results)
}

pub fn preview_ops(worktree_root: &Path, ops: &[FileOp]) -> Result<Vec<PlannedChange>, PilotError> {
    ops.iter().map(|op| preview_op(worktree_root, op)).collect()
}

pub async fn apply_ops_async(
    worktree_root: std::path::PathBuf, ops: Vec<FileOp>, limits: ApplyLimits,
) -> Result<Vec<ApplyResult>, PilotError> {
    tokio::task::spawn_blocking(move || apply_ops(&worktree_root, &ops, &limits))
        .await
        .map_err(|je| PilotError::Apply(ApplyError::TaskJoin {
            operation: "apply_ops".into(),
            reason: if je.is_panic() { "task panicked".into() } else { "task cancelled".into() },
        }))?
}

pub async fn preview_ops_async(
    worktree_root: std::path::PathBuf, ops: Vec<FileOp>,
) -> Result<Vec<PlannedChange>, PilotError> {
    tokio::task::spawn_blocking(move || preview_ops(&worktree_root, &ops))
        .await
        .map_err(|je| PilotError::Apply(ApplyError::TaskJoin {
            operation: "preview_ops".into(),
            reason: if je.is_panic() { "task panicked".into() } else { "task cancelled".into() },
        }))?
}

fn validate_limits(ops: &[FileOp], limits: &ApplyLimits) -> Result<(), PilotError> {
    if ops.len() > limits.max_ops_per_block {
        return Err(PilotError::Limits(format!("ops block contains {} operations (max {})", ops.len(), limits.max_ops_per_block)));
    }
    let mut total: usize = 0;
    for op in ops {
        let len = match op {
            FileOp::Write { content, .. } => content.len(),
            FileOp::Replace { find, replace, .. } => find.len() + replace.len(),
            FileOp::Delete { .. } => 0,
        };
        if len > limits.max_file_size {
            return Err(PilotError::Limits(format!("content for '{}' is {} bytes (max {})", op.path(), len, limits.max_file_size)));
        }
        total += len;
    }
    if total > limits.max_total_content {
        return Err(PilotError::Limits(format!("total content is {} bytes (max {})", total, limits.max_total_content)));
    }
    Ok(())
}

fn create_backups(worktree_root: &Path, ops: &[FileOp]) -> Result<Vec<Backup>, PilotError> {
    let mut backups = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for op in ops {
        let rel_path = op.path();
        if seen.contains(rel_path) { continue; }
        seen.insert(rel_path.to_string());
        let abs_path = validate_path(worktree_root, rel_path).map_err(|reason| PilotError::PathEscape {
            path: rel_path.to_string(), root: worktree_root.display().to_string(), reason,
        })?;
        let original_content = if abs_path.exists() {
            Some(fs::read_to_string(&abs_path).map_err(|e| PilotError::Apply(ApplyError::Io {
                path: rel_path.to_string(), operation: "read_for_backup".into(), source: e,
            }))?)
        } else { None };
        backups.push(Backup { rel_path: rel_path.to_string(), original_content });
    }
    Ok(backups)
}

fn rollback(worktree_root: &Path, backups: &[Backup], results: &[ApplyResult]) {
    for result in results {
        let backup = backups.iter().find(|b| b.rel_path == result.path);
        match backup {
            Some(b) => match &b.original_content {
                Some(content) => {
                    let abs_path = match validate_path(worktree_root, &b.rel_path) { Ok(p) => p, Err(_) => continue };
                    if let Err(e) = fs::write(&abs_path, content) {
                        tracing::error!(path = &b.rel_path, error = %e, "rollback: failed to restore");
                    }
                }
                None => {
                    let abs_path = match validate_path(worktree_root, &b.rel_path) { Ok(p) => p, Err(_) => continue };
                    if abs_path.exists() { let _ = fs::remove_file(&abs_path); }
                }
            },
            None => {
                let abs_path = match validate_path(worktree_root, &result.path) { Ok(p) => p, Err(_) => continue };
                if abs_path.exists() { let _ = fs::remove_file(&abs_path); }
            }
        }
    }
}

fn apply_op_atomic(worktree_root: &Path, op: &FileOp) -> Result<ApplyResult, PilotError> {
    match op {
        FileOp::Write { path, content } => apply_write_atomic(worktree_root, path, content),
        FileOp::Replace { path, find, replace } => apply_replace_atomic(worktree_root, path, find, replace),
        FileOp::Delete { path } => apply_delete(worktree_root, path),
    }
}

fn apply_write_atomic(worktree_root: &Path, rel_path: &str, content: &str) -> Result<ApplyResult, PilotError> {
    let abs_path = validate_path(worktree_root, rel_path).map_err(|reason| PilotError::PathEscape {
        path: rel_path.to_string(), root: worktree_root.display().to_string(), reason,
    })?;
    if let Some(parent) = abs_path.parent() {
        fs::create_dir_all(parent).map_err(|e| PilotError::Apply(ApplyError::Io {
            path: rel_path.to_string(), operation: "create_dir_all".into(), source: e,
        }))?;
    }
    let existed = abs_path.exists();
    let tmp_path = abs_path.with_extension("glyim-tmp");
    fs::write(&tmp_path, content).map_err(|e| PilotError::Apply(ApplyError::Io {
        path: rel_path.to_string(), operation: "write_tmp".into(), source: e,
    }))?;
    fs::rename(&tmp_path, &abs_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        PilotError::Apply(ApplyError::Io { path: rel_path.to_string(), operation: "rename".into(), source: e })
    })?;
    Ok(ApplyResult { path: rel_path.to_string(), action: if existed { ApplyAction::Modified } else { ApplyAction::Created } })
}

fn apply_replace_atomic(worktree_root: &Path, rel_path: &str, find: &str, replace: &str) -> Result<ApplyResult, PilotError> {
    let abs_path = validate_path(worktree_root, rel_path).map_err(|reason| PilotError::PathEscape {
        path: rel_path.to_string(), root: worktree_root.display().to_string(), reason,
    })?;
    if !abs_path.exists() {
        return Err(PilotError::Apply(ApplyError::FileNotFound(rel_path.to_string())));
    }
    let existing = fs::read_to_string(&abs_path).map_err(|e| PilotError::Apply(ApplyError::Io {
        path: rel_path.to_string(), operation: "read".into(), source: e,
    }))?;
    let count = existing.matches(find).count();
    if count == 0 { return Err(PilotError::Apply(ApplyError::FindNotFound { path: rel_path.to_string() })); }
    if count > 1 { return Err(PilotError::Apply(ApplyError::FindAmbiguous { path: rel_path.to_string(), count })); }
    let new_content = existing.replacen(find, replace, 1);
    let tmp_path = abs_path.with_extension("glyim-tmp");
    fs::write(&tmp_path, &new_content).map_err(|e| PilotError::Apply(ApplyError::Io {
        path: rel_path.to_string(), operation: "write_tmp".into(), source: e,
    }))?;
    fs::rename(&tmp_path, &abs_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        PilotError::Apply(ApplyError::Io { path: rel_path.to_string(), operation: "rename".into(), source: e })
    })?;
    Ok(ApplyResult { path: rel_path.to_string(), action: ApplyAction::Modified })
}

fn apply_delete(worktree_root: &Path, rel_path: &str) -> Result<ApplyResult, PilotError> {
    let abs_path = validate_path(worktree_root, rel_path).map_err(|reason| PilotError::PathEscape {
        path: rel_path.to_string(), root: worktree_root.display().to_string(), reason,
    })?;
    if !abs_path.exists() { return Err(PilotError::Apply(ApplyError::FileNotFound(rel_path.to_string()))); }
    fs::remove_file(&abs_path).map_err(|e| PilotError::Apply(ApplyError::Io {
        path: rel_path.to_string(), operation: "delete".into(), source: e,
    }))?;
    Ok(ApplyResult { path: rel_path.to_string(), action: ApplyAction::Deleted })
}

fn preview_op(worktree_root: &Path, op: &FileOp) -> Result<PlannedChange, PilotError> {
    match op {
        FileOp::Write { path, .. } => {
            let abs_path = validate_path(worktree_root, path).map_err(|r| PilotError::PathEscape {
                path: path.clone(), root: worktree_root.display().to_string(), reason: r,
            })?;
            let exists = abs_path.exists();
            Ok(PlannedChange {
                path: path.clone(),
                action: if exists { PlannedAction::Overwrite } else { PlannedAction::Create },
                current_content_summary: fs::metadata(&abs_path).ok().map(|m| format!("existing file ({} bytes)", m.len())),
            })
        }
        FileOp::Replace { path, .. } => {
            let abs_path = validate_path(worktree_root, path).map_err(|r| PilotError::PathEscape {
                path: path.clone(), root: worktree_root.display().to_string(), reason: r,
            })?;
            if !abs_path.exists() { return Err(PilotError::Apply(ApplyError::FileNotFound(path.clone()))); }
            Ok(PlannedChange { path: path.clone(), action: PlannedAction::Modify, current_content_summary: None })
        }
        FileOp::Delete { path } => {
            let abs_path = validate_path(worktree_root, path).map_err(|r| PilotError::PathEscape {
                path: path.clone(), root: worktree_root.display().to_string(), reason: r,
            })?;
            if !abs_path.exists() { return Err(PilotError::Apply(ApplyError::FileNotFound(path.clone()))); }
            Ok(PlannedChange { path: path.clone(), action: PlannedAction::Delete, current_content_summary: None })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_apply_and_rollback() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "original a").unwrap();
        let ops = vec![
            FileOp::Write { path: "src/a.rs".into(), content: "modified a".into() },
            FileOp::Replace { path: "src/a.rs".into(), find: "nonexistent".into(), replace: "x".into() },
        ];
        let result = apply_ops(dir.path(), &ops, &ApplyLimits::default());
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(dir.path().join("src/a.rs")).unwrap(), "original a");
    }

    #[test]
    fn test_apply_limits() {
        let dir = tempfile::tempdir().unwrap();
        let limits = ApplyLimits { max_ops_per_block: 1, ..ApplyLimits::default() };
        let ops = vec![
            FileOp::Write { path: "a.rs".into(), content: "a".into() },
            FileOp::Write { path: "b.rs".into(), content: "b".into() },
        ];
        assert!(matches!(apply_ops(dir.path(), &ops, &limits).unwrap_err(), PilotError::Limits(_)));
    }
}
```

## src/config/mod.rs

```rust
pub mod types;
use crate::error::PilotError;
use std::path::Path;
pub use types::*;

pub fn load_config(project_root: &Path) -> Result<PilotConfig, PilotError> {
    let config_path = project_root.join(".glyim-pilot.toml");
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| PilotError::Config(format!("failed to read config: {e}")))?;
    toml::from_str(&content).map_err(|e| PilotError::Config(format!("failed to parse config: {e}")))
}
```

## src/config/types.rs

```rust
use crate::domain_types::{ApplyLimits, BannedPattern, DependencyRule};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PilotConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub execution: ExecutionConfig,
    #[serde(default)]
    pub gates: GatesConfig,
    #[serde(default)]
    pub context: ContextConfig,
    #[serde(default)]
    pub dispatch: DispatchConfig,
    #[serde(default)]
    pub limits: ApplyLimits,
}

impl PilotConfig {
    pub fn default_for_testing() -> Self {
        let mut providers = HashMap::new();
        providers.insert("test-provider".into(), ProviderConfig::default());
        Self {
            server: ServerConfig::default(), defaults: DefaultsConfig::default(),
            providers, execution: ExecutionConfig::default(), gates: GatesConfig::default(),
            context: ContextConfig::default(), dispatch: DispatchConfig::default(),
            limits: ApplyLimits::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig { #[serde(default = "default_port")] pub port: u16, #[serde(default = "default_host")] pub host: String }
fn default_port() -> u16 { 8420 }
fn default_host() -> String { "127.0.0.1".into() }
impl Default for ServerConfig { fn default() -> Self { Self { port: default_port(), host: default_host() } } }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DefaultsConfig {
    #[serde(default)] pub provider: String,
    #[serde(default)] pub auto_execute: bool,
    #[serde(default = "default_max_turns")] pub max_turns: u32,
    #[serde(default = "default_true")] pub retry_on_rate_limit: bool,
    #[serde(default = "default_retry_max_wait")] pub retry_max_wait: u64,
}
fn default_max_turns() -> u32 { 50 }
fn default_true() -> bool { true }
fn default_retry_max_wait() -> u64 { 120 }
impl Default for DefaultsConfig {
    fn default() -> Self { Self { provider: String::new(), auto_execute: false, max_turns: default_max_turns(), retry_on_rate_limit: true, retry_max_wait: default_retry_max_wait() } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    #[serde(default = "default_true")] pub enabled: bool,
    #[serde(default)] pub url: String,
    #[serde(default = "default_max_concurrent")] pub max_concurrent: usize,
    #[serde(default = "default_cooldown")] pub rate_limit_cooldown: u64,
    #[serde(default)] pub error_patterns: Vec<String>,
    #[serde(default = "default_input_selector")] pub input_selector: String,
    #[serde(default = "default_send_selector")] pub send_selector: String,
    #[serde(default)] pub streaming_indicator: String,
    #[serde(default)] pub assistant_selector: String,
    #[serde(default = "default_code_block_selector")] pub code_block_selector: String,
}
fn default_max_concurrent() -> usize { 3 }
fn default_cooldown() -> u64 { 60 }
fn default_input_selector() -> String { "textarea".into() }
fn default_send_selector() -> String { "button[type='submit']".into() }
fn default_code_block_selector() -> String { "pre code".into() }
impl Default for ProviderConfig {
    fn default() -> Self { Self { enabled: true, url: String::new(), max_concurrent: default_max_concurrent(), rate_limit_cooldown: default_cooldown(), error_patterns: Vec::new(), input_selector: default_input_selector(), send_selector: default_send_selector(), streaming_indicator: String::new(), assistant_selector: String::new(), code_block_selector: default_code_block_selector() } }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecutionConfig {
    #[serde(default = "default_worktree_base")] pub worktree_base: String,
    #[serde(default = "default_require_confirmation")] pub require_confirmation: String,
    #[serde(default = "default_dangerous_patterns")] pub dangerous_patterns: Vec<String>,
    #[serde(default = "default_max_fix_rounds")] pub max_fix_rounds: u32,
    #[serde(default = "default_command_timeout")] pub command_timeout: u64,
    #[serde(default = "default_branch")] pub default_branch: String,
    #[serde(default = "default_branch_version")] pub branch_version: String,
}
fn default_worktree_base() -> String { "../glyim-worktrees".into() }
fn default_require_confirmation() -> String { "first".into() }
fn default_dangerous_patterns() -> Vec<String> { vec!["rm -rf".into(), "git push".into(), "git reset --hard".into(), "cargo publish".into(), "sudo".into()] }
fn default_max_fix_rounds() -> u32 { 5 }
fn default_command_timeout() -> u64 { 300 }
fn default_branch() -> String { "main".into() }
fn default_branch_version() -> String { "v0.1.0".into() }
impl Default for ExecutionConfig { fn default() -> Self { Self { worktree_base: default_worktree_base(), require_confirmation: default_require_confirmation(), dangerous_patterns: default_dangerous_patterns(), max_fix_rounds: default_max_fix_rounds(), command_timeout: default_command_timeout(), default_branch: default_branch(), branch_version: default_branch_version() } } }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GateLevel { Relaxed, Normal, Strict, Production }
impl Default for GateLevel { fn default() -> Self { Self::Normal } }
impl std::fmt::Display for GateLevel { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { match self { Self::Relaxed => write!(f, "relaxed"), Self::Normal => write!(f, "normal"), Self::Strict => write!(f, "strict"), Self::Production => write!(f, "production") } } }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GatesConfig {
    #[serde(default)] pub level: GateLevel,
    #[serde(default)] pub commit: CommitGatesConfig,
    #[serde(default)] pub done: DoneGatesConfig,
    #[serde(default)] pub banned_patterns: Vec<BannedPattern>,
    #[serde(default)] pub architecture_rules: Vec<DependencyRule>,
}
impl Default for GatesConfig { fn default() -> Self { Self { level: GateLevel::default(), commit: CommitGatesConfig::default(), done: DoneGatesConfig::default(), banned_patterns: Vec::new(), architecture_rules: Vec::new() } } }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitGatesConfig {
    pub fmt: Option<bool>, pub check: Option<bool>, pub clippy: Option<bool>,
    pub test: Option<bool>, pub banned_patterns: Option<bool>,
    pub architecture: Option<bool>, pub contracts: Option<bool>,
}
impl Default for CommitGatesConfig { fn default() -> Self { Self { fmt: None, check: None, clippy: None, test: None, banned_patterns: None, architecture: None, contracts: None } } }

impl CommitGatesConfig {
    pub fn resolve(&self, level: GateLevel, default_branch: String, branch_version: String) -> ResolvedCommitGates {
        let d = level.commit_defaults();
        ResolvedCommitGates {
            fmt: self.fmt.unwrap_or(d.fmt), check: self.check.unwrap_or(d.check),
            clippy: self.clippy.unwrap_or(d.clippy), test: self.test.unwrap_or(d.test),
            banned_patterns: self.banned_patterns.unwrap_or(d.banned_patterns),
            architecture: self.architecture.unwrap_or(d.architecture),
            contracts: self.contracts.unwrap_or(d.contracts),
            default_branch, branch_version,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedCommitGates {
    pub fmt: bool, pub check: bool, pub clippy: bool, pub test: bool,
    pub banned_patterns: bool, pub architecture: bool, pub contracts: bool,
    pub default_branch: String, pub branch_version: String,
}

struct CommitDefaults { fmt: bool, check: bool, clippy: bool, test: bool, banned_patterns: bool, architecture: bool, contracts: bool }
impl GateLevel {
    fn commit_defaults(self) -> CommitDefaults {
        match self {
            Self::Relaxed => CommitDefaults { fmt: true, check: true, clippy: false, test: false, banned_patterns: false, architecture: false, contracts: false },
            Self::Normal => CommitDefaults { fmt: true, check: true, clippy: true, test: true, banned_patterns: false, architecture: false, contracts: false },
            Self::Strict | Self::Production => CommitDefaults { fmt: true, check: true, clippy: true, test: true, banned_patterns: true, architecture: true, contracts: true },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DoneGatesConfig {
    pub dead_code: Option<bool>, pub coverage: Option<bool>,
    #[serde(default = "default_coverage_min")] pub coverage_min: f64,
    pub mutation: Option<bool>, #[serde(default = "default_mutation_kill_rate")] pub mutation_kill_rate: f64,
    pub workspace_check: Option<bool>, pub audit: Option<bool>, pub self_review: Option<bool>,
}
fn default_coverage_min() -> f64 { 0.80 }
fn default_mutation_kill_rate() -> f64 { 0.75 }
impl Default for DoneGatesConfig { fn default() -> Self { Self { dead_code: None, coverage: None, coverage_min: default_coverage_min(), mutation: None, mutation_kill_rate: default_mutation_kill_rate(), workspace_check: None, audit: None, self_review: None } } }

impl DoneGatesConfig {
    pub fn resolve(&self, level: GateLevel) -> ResolvedDoneGates {
        let d = level.done_defaults();
        ResolvedDoneGates {
            dead_code: self.dead_code.unwrap_or(d.dead_code), coverage: self.coverage.unwrap_or(d.coverage),
            coverage_min: self.coverage_min, mutation: self.mutation.unwrap_or(d.mutation),
            mutation_kill_rate: self.mutation_kill_rate, workspace_check: self.workspace_check.unwrap_or(d.workspace_check),
            audit: self.audit.unwrap_or(d.audit), self_review: self.self_review.unwrap_or(d.self_review),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedDoneGates {
    pub dead_code: bool, pub coverage: bool, pub coverage_min: f64,
    pub mutation: bool, pub mutation_kill_rate: f64,
    pub workspace_check: bool, pub audit: bool, pub self_review: bool,
}

struct DoneDefaults { dead_code: bool, coverage: bool, coverage_min: f64, mutation: bool, mutation_kill_rate: f64, workspace_check: bool, audit: bool, self_review: bool }
impl GateLevel {
    fn done_defaults(self) -> DoneDefaults {
        match self {
            Self::Relaxed | Self::Normal => DoneDefaults { dead_code: false, coverage: false, coverage_min: 0.80, mutation: false, mutation_kill_rate: 0.75, workspace_check: false, audit: false, self_review: false },
            Self::Strict => DoneDefaults { dead_code: true, coverage: false, coverage_min: 0.80, mutation: false, mutation_kill_rate: 0.75, workspace_check: true, audit: false, self_review: false },
            Self::Production => DoneDefaults { dead_code: true, coverage: true, coverage_min: 0.80, mutation: true, mutation_kill_rate: 0.75, workspace_check: true, audit: true, self_review: true },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContextConfig { #[serde(default = "default_max_context_tokens")] pub max_context_tokens: usize, #[serde(default)] pub providers: HashMap<String, ProviderContextConfig> }
fn default_max_context_tokens() -> usize { 15000 }
impl Default for ContextConfig { fn default() -> Self { Self { max_context_tokens: default_max_context_tokens(), providers: HashMap::new() } } }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderContextConfig { #[serde(default = "default_max_context_tokens")] pub max_context_tokens: usize }
impl Default for ProviderContextConfig { fn default() -> Self { Self { max_context_tokens: default_max_context_tokens() } } }

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DispatchConfig { #[serde(default = "default_strategy")] pub strategy: String, #[serde(default = "default_true")] pub fallback_on_rate_limit: bool, #[serde(default = "default_max_reassign")] pub max_reassign_attempts: u32 }
fn default_strategy() -> String { "most_slots_first".into() }
fn default_max_reassign() -> u32 { 2 }
impl Default for DispatchConfig { fn default() -> Self { Self { strategy: default_strategy(), fallback_on_rate_limit: true, max_reassign_attempts: default_max_reassign() } } }
```

## src/git_ops/mod.rs

```rust
pub mod worktree;
pub use worktree::{
    create_worktree, commit_all, emergency_wip_commit, push_branch, create_pr,
    status_porcelain, diff_main, log_oneline, diff_name_only, remove_worktree,
    detect_default_branch,
};
```

## src/git_ops/worktree.rs

```rust
use crate::error::PilotError;
use crate::process::run_timed_command;
use std::path::{Path, PathBuf};

pub async fn create_worktree(
    repo_root: &Path, worktree_base: &Path, stream_id: &str,
    default_branch: &str, branch_version: &str, timeout_secs: u64,
) -> Result<PathBuf, PilotError> {
    let worktree_dir = worktree_base.join(format!("stream-{stream_id}"));
    let branch_name = format!("stream-{stream_id}/{branch_version}");
    let args = &["worktree", "add", "--detach", &worktree_dir.to_string_lossy(), default_branch];
    let output = run_timed_command("git", args, repo_root, timeout_secs).await.map_err(|e| PilotError::Git(e.to_string()))?;
    if !output.status.success() {
        return Err(PilotError::Git(format!("git worktree add failed: {}", String::from_utf8_lossy(&output.stderr).trim())));
    }
    let checkout_args = &["checkout", "-b", &branch_name];
    let output = run_timed_command("git", checkout_args, &worktree_dir, timeout_secs).await.map_err(|e| PilotError::Git(e.to_string()))?;
    if !output.status.success() {
        return Err(PilotError::Git(format!("git checkout -b failed: {}", String::from_utf8_lossy(&output.stderr).trim())));
    }
    Ok(worktree_dir)
}

pub async fn commit_all(worktree_dir: &Path, stream_id: &str, message: &str, timeout_secs: u64) -> Result<(), PilotError> {
    let commit_msg = format!("stream-{stream_id}: {message}");
    let output = run_timed_command("git", &["add", "-A"], worktree_dir, timeout_secs).await.map_err(|e| PilotError::Git(e.to_string()))?;
    if !output.status.success() {
        return Err(PilotError::Git(format!("git add failed: {}", String::from_utf8_lossy(&output.stderr).trim())));
    }
    let output = run_timed_command("git", &["commit", "-m", &commit_msg], worktree_dir, timeout_secs).await.map_err(|e| PilotError::Git(e.to_string()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.contains("nothing to commit") { return Ok(()); }
        return Err(PilotError::Git(format!("git commit failed: {stderr}")));
    }
    Ok(())
}

pub async fn emergency_wip_commit(worktree_dir: &Path, stream_id: &str, timeout_secs: u64) -> Result<(), PilotError> {
    commit_all(worktree_dir, stream_id, "WIP: emergency commit — fix rounds exceeded", timeout_secs).await
}

pub async fn push_branch(worktree_dir: &Path, stream_id: &str, branch_version: &str, timeout_secs: u64) -> Result<(), PilotError> {
    let branch_name = format!("stream-{stream_id}/{branch_version}");
    let output = run_timed_command("git", &["push", "-u", "origin", &branch_name], worktree_dir, timeout_secs).await.map_err(|e| PilotError::Git(e.to_string()))?;
    if !output.status.success() { return Err(PilotError::Git(format!("git push failed: {}", String::from_utf8_lossy(&output.stderr).trim()))); }
    Ok(())
}

pub async fn create_pr(worktree_dir: &Path, stream_id: &str, default_branch: &str, branch_version: &str, title: &str, body: &str, timeout_secs: u64) -> Result<String, PilotError> {
    let branch_name = format!("stream-{stream_id}/{branch_version}");
    let output = run_timed_command("gh", &["pr", "create", "--base", default_branch, "--head", &branch_name, "--title", title, "--body", body], worktree_dir, timeout_secs).await.map_err(|e| PilotError::Git(e.to_string()))?;
    if !output.status.success() { return Err(PilotError::Git(format!("gh pr create failed: {}", String::from_utf8_lossy(&output.stderr).trim()))); }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn status_porcelain(worktree_dir: &Path, timeout_secs: u64) -> Result<String, PilotError> {
    let output = run_timed_command("git", &["status", "--porcelain"], worktree_dir, timeout_secs).await.map_err(|e| PilotError::Git(e.to_string()))?;
    if !output.status.success() { return Err(PilotError::Git(format!("git status failed: {}", String::from_utf8_lossy(&output.stderr).trim()))); }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn diff_main(worktree_dir: &Path, default_branch: &str, timeout_secs: u64) -> Result<String, PilotError> {
    let output = run_timed_command("git", &["diff", &format!("{default_branch}..HEAD")], worktree_dir, timeout_secs).await.map_err(|e| PilotError::Git(e.to_string()))?;
    if !output.status.success() { return Err(PilotError::Git(format!("git diff failed: {}", String::from_utf8_lossy(&output.stderr).trim()))); }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub async fn log_oneline(worktree_dir: &Path, default_branch: &str, timeout_secs: u64) -> Result<String, PilotError> {
    let output = run_timed_command("git", &["log", &format!("{default_branch}..HEAD"), "--oneline"], worktree_dir, timeout_secs).await.map_err(|e| PilotError::Git(e.to_string()))?;
    if !output.status.success() { return Err(PilotError::Git(format!("git log failed: {}", String::from_utf8_lossy(&output.stderr).trim()))); }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn diff_name_only(worktree_dir: &Path, default_branch: &str, timeout_secs: u64) -> Result<String, PilotError> {
    let output = run_timed_command("git", &["diff", "--name-only", &format!("{default_branch}..HEAD")], worktree_dir, timeout_secs).await.map_err(|e| PilotError::Git(e.to_string()))?;
    if !output.status.success() { return Err(PilotError::Git(format!("git diff --name-only failed: {}", String::from_utf8_lossy(&output.stderr).trim()))); }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn remove_worktree(repo_root: &Path, worktree_dir: &Path, timeout_secs: u64) -> Result<(), PilotError> {
    let output = run_timed_command("git", &["worktree", "remove", &worktree_dir.to_string_lossy(), "--force"], repo_root, timeout_secs).await.map_err(|e| PilotError::Git(e.to_string()))?;
    if !output.status.success() { return Err(PilotError::Git(format!("git worktree remove failed: {}", String::from_utf8_lossy(&output.stderr).trim()))); }
    Ok(())
}

pub async fn detect_default_branch(repo_root: &Path, fallback: &str, timeout_secs: u64) -> String {
    match run_timed_command("git", &["symbolic-ref", "refs/remotes/origin/HEAD"], repo_root, timeout_secs).await {
        Ok(output) if output.status.success() => {
            let ref_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            ref_path.strip_prefix("refs/remotes/origin/").map(|s| s.to_string()).unwrap_or_else(|| fallback.to_string())
        }
        _ => fallback.to_string(),
    }
}
```

## src/gates/mod.rs

```rust
pub mod types;
pub mod helpers;
pub mod fmt_check;
pub mod fmt_fix;
pub mod check;
pub mod clippy;
pub mod test_gate;
pub mod banned_pattern;
pub mod architecture;
pub mod contracts;
pub mod commit_pipeline;
pub mod dead_code;
pub mod coverage;
pub mod mutation;
pub mod workspace_check;
pub mod audit;
pub mod self_review;
pub mod done_pipeline;

use crate::error::PilotError;
use crate::gates::types::GateContext;
use async_trait::async_trait;

pub use types::{GateResult, GateSideEffect, PipelineResult};

#[async_trait]
pub trait Gate: Send + Sync {
    fn name(&self) -> &str;
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError>;
}
```

## src/gates/types.rs

**Fix #8 from critique**: `GateContext` now includes `changed_files` computed once and shared. **Fix #10 from critique**: `PipelineResult::failure_message()` includes side effects from passing gates.

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Context passed to every gate. Includes pre-computed changed files
/// so gates don't each make their own git call.
#[derive(Debug, Clone)]
pub struct GateContext {
    pub worktree_dir: PathBuf,
    pub project_root: PathBuf,
    pub default_branch: String,
    pub branch_version: String,
    pub timeout_secs: u64,
    /// Pre-computed list of changed files (via git diff --name-only).
    /// Computed once by the pipeline and shared across all gates.
    /// Empty if the diff could not be computed (falls back to full scan).
    pub changed_files: Vec<String>,
}

impl GateContext {
    pub fn new(
        worktree_dir: PathBuf, project_root: PathBuf,
        default_branch: String, branch_version: String,
        timeout_secs: u64, changed_files: Vec<String>,
    ) -> Self {
        Self { worktree_dir, project_root, default_branch, branch_version, timeout_secs, changed_files }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateSideEffect {
    pub description: String,
    pub affected_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub gate_name: String,
    pub passed: bool,
    pub message: String,
    pub details: Option<String>,
    pub side_effects: Vec<GateSideEffect>,
}

impl GateResult {
    pub fn pass(name: impl Into<String>) -> Self {
        Self { gate_name: name.into(), passed: true, message: "passed".into(), details: None, side_effects: Vec::new() }
    }
    pub fn pass_with_note(name: impl Into<String>, note: impl Into<String>) -> Self {
        Self { gate_name: name.into(), passed: true, message: note.into(), details: None, side_effects: Vec::new() }
    }
    pub fn pass_with_side_effects(
        name: impl Into<String>, note: impl Into<String>,
        details: impl Into<String>, side_effects: Vec<GateSideEffect>,
    ) -> Self {
        Self { gate_name: name.into(), passed: true, message: note.into(), details: Some(details.into()), side_effects }
    }
    pub fn fail(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self { gate_name: name.into(), passed: false, message: message.into(), details: None, side_effects: Vec::new() }
    }
    pub fn fail_with_details(name: impl Into<String>, message: impl Into<String>, details: impl Into<String>) -> Self {
        Self { gate_name: name.into(), passed: false, message: message.into(), details: Some(details.into()), side_effects: Vec::new() }
    }
    pub fn has_side_effects(&self) -> bool { !self.side_effects.is_empty() }
}

#[derive(Debug, Clone)]
pub struct PipelineResult {
    pub gates: Vec<GateResult>,
    pub passed: bool,
}

impl PipelineResult {
    pub fn from_gates(gates: Vec<GateResult>) -> Self {
        let passed = gates.iter().all(|g| g.passed);
        Self { gates, passed }
    }
    pub fn first_failure(&self) -> Option<&GateResult> {
        self.gates.iter().find(|g| !g.passed)
    }

    /// Failure message including side effects from gates that passed
    /// but modified the worktree. The AI needs to know that auto-fixes
    /// were applied even when a later gate fails.
    pub fn failure_message(&self) -> String {
        if let Some(fail) = self.first_failure() {
            let mut msg = format!("**{} failed**: {}", fail.gate_name, fail.message);
            if let Some(details) = &fail.details {
                msg = format!("{msg}\n\n```\n{details}\n```");
            }
            // Include side effects from passing gates
            let side_effects: Vec<&GateSideEffect> = self.gates.iter()
                .filter(|g| g.passed && g.has_side_effects())
                .flat_map(|g| &g.side_effects)
                .collect();
            if !side_effects.is_empty() {
                msg.push_str("\n\n**Note: auto-fixes were applied before this failure:**\n");
                for se in side_effects {
                    msg.push_str(&format!("- {}\n", se.description));
                    if !se.affected_files.is_empty() {
                        msg.push_str(&format!("  Files: {}\n", se.affected_files.join(", ")));
                    }
                }
            }
            msg
        } else {
            String::new()
        }
    }
}
```

## src/gates/helpers.rs

```rust
use crate::error::PilotError;
use crate::process::run_timed_command;
use std::path::Path;

pub async fn run_gate_command(
    program: &str, args: &[&str], cwd: &Path, timeout_secs: u64, gate_name: &str,
) -> Result<std::process::Output, PilotError> {
    run_timed_command(program, args, cwd, timeout_secs)
        .await
        .map_err(|e| PilotError::Gate { gate: gate_name.into(), message: e.to_string() })
}

pub fn strip_ansi(s: &str) -> String { strip_ansi_escapes::strip_str(s) }

pub fn trim_errors_and_warnings(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= 50 { return output.to_string(); }
    let mut relevant = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("error") || trimmed.starts_with("warning") {
            let start = i.saturating_sub(2);
            let end = (i + 5).min(lines.len());
            for j in start..end { relevant.push(lines[j]); }
            relevant.push("...");
        }
    }
    if relevant.is_empty() { lines[lines.len() - 50..].join("\n") } else { relevant.join("\n") }
}

pub fn is_command_not_found(stdout: &str, stderr: &str) -> bool {
    let combined = format!("{stdout}\n{stderr}").to_lowercase();
    combined.contains("command not found") || combined.contains("no such command") || combined.contains("not found")
}

pub fn trim_test_failures(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= 80 { return output.to_string(); }
    let mut relevant = Vec::new();
    let mut in_failures = false;
    for line in &lines {
        if line.trim().starts_with("failures:") { in_failures = true; }
        if in_failures { relevant.push(*line); if relevant.len() > 60 { break; } }
    }
    for line in lines.iter().rev() {
        if line.contains("test result:") { relevant.push(*line); break; }
    }
    if relevant.is_empty() { lines[lines.len() - 60..].join("\n") } else { relevant.join("\n") }
}

/// Get changed files using a pre-computed list from GateContext.
/// This is a convenience for gate implementations.
pub fn get_changed_files_from_ctx(ctx: &crate::gates::types::GateContext) -> &[String] {
    &ctx.changed_files
}
```

## src/gates/fmt_check.rs

```rust
use crate::error::PilotError;
use crate::gates::helpers::run_gate_command;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct FmtCheckGate;

#[async_trait]
impl Gate for FmtCheckGate {
    fn name(&self) -> &str { "fmt" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_gate_command("cargo", &["fmt", "--", "--check"], &ctx.worktree_dir, ctx.timeout_secs, "fmt").await?;
        if output.status.success() {
            Ok(GateResult::pass("fmt"))
        } else {
            let combined = crate::gates::helpers::strip_ansi(&String::from_utf8_lossy(&output.stdout));
            Ok(GateResult::fail_with_details("fmt", "formatting check failed — run `cargo fmt` to fix", crate::gates::helpers::trim_errors_and_warnings(&combined)))
        }
    }
}
```

## src/gates/fmt_fix.rs

```rust
use crate::error::PilotError;
use crate::gates::helpers::run_gate_command;
use crate::gates::types::{GateContext, GateResult, GateSideEffect};

pub async fn run_fmt_fix(ctx: &GateContext) -> Result<GateResult, PilotError> {
    let output = run_gate_command("cargo", &["fmt"], &ctx.worktree_dir, ctx.timeout_secs, "fmt_fix").await?;
    if !output.status.success() {
        let stderr = crate::gates::helpers::strip_ansi(&String::from_utf8_lossy(&output.stderr));
        return Ok(GateResult::fail_with_details("fmt_fix", "cargo fmt failed to apply formatting", stderr));
    }
    let changed_files_result = crate::git_ops::diff_name_only(
        &ctx.worktree_dir, &ctx.default_branch, ctx.timeout_secs,
    ).await;
    let changed_files: Vec<String> = match changed_files_result {
        Ok(output) => output.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect(),
        Err(e) => { tracing::warn!("fmt_fix: could not get changed files: {e}"); Vec::new() }
    };
    Ok(GateResult::pass_with_side_effects(
        "fmt_fix", "auto-fixed: cargo fmt applied changes (not committed)",
        format!("Changed files:\n{}", changed_files.join("\n")),
        vec![GateSideEffect { description: "auto-fixed formatting via cargo fmt".into(), affected_files: changed_files }],
    ))
}
```

## src/gates/check.rs, clippy.rs, test_gate.rs, dead_code.rs, coverage.rs, mutation.rs, workspace_check.rs, audit.rs

```rust
// check.rs
use crate::error::PilotError;
use crate::gates::helpers::{run_gate_command, strip_ansi, trim_errors_and_warnings};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct CheckGate;
#[async_trait]
impl Gate for CheckGate {
    fn name(&self) -> &str { "check" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_gate_command("cargo", &["check"], &ctx.worktree_dir, ctx.timeout_secs, "check").await?;
        if output.status.success() { Ok(GateResult::pass("check")) }
        else { Ok(GateResult::fail_with_details("check", "compilation failed", trim_errors_and_warnings(&strip_ansi(&String::from_utf8_lossy(&output.stderr))))) }
    }
}
```

```rust
// clippy.rs
use crate::error::PilotError;
use crate::gates::helpers::{run_gate_command, strip_ansi, trim_errors_and_warnings};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct ClippyGate;
#[async_trait]
impl Gate for ClippyGate {
    fn name(&self) -> &str { "clippy" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_gate_command("cargo", &["clippy", "--", "-D", "warnings"], &ctx.worktree_dir, ctx.timeout_secs, "clippy").await?;
        if output.status.success() { Ok(GateResult::pass("clippy")) }
        else { Ok(GateResult::fail_with_details("clippy", "clippy warnings found", trim_errors_and_warnings(&strip_ansi(&String::from_utf8_lossy(&output.stderr))))) }
    }
}
```

```rust
// test_gate.rs
use crate::error::PilotError;
use crate::gates::helpers::{run_gate_command, strip_ansi, trim_test_failures};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct TestGate;
#[async_trait]
impl Gate for TestGate {
    fn name(&self) -> &str { "test" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_gate_command("cargo", &["test"], &ctx.worktree_dir, ctx.timeout_secs, "test").await?;
        if output.status.success() { Ok(GateResult::pass("test")) }
        else {
            let combined = format!("{}\n{}", strip_ansi(&String::from_utf8_lossy(&output.stdout)), strip_ansi(&String::from_utf8_lossy(&output.stderr)));
            Ok(GateResult::fail_with_details("test", "test failures detected", trim_test_failures(&combined)))
        }
    }
}
```

```rust
// dead_code.rs
use crate::error::PilotError;
use crate::gates::helpers::{run_gate_command, strip_ansi};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct DeadCodeGate;
#[async_trait]
impl Gate for DeadCodeGate {
    fn name(&self) -> &str { "dead_code" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_gate_command("cargo", &["check", "--all-targets", "--", "-W", "dead_code", "-W", "unused_imports"], &ctx.worktree_dir, ctx.timeout_secs, "dead_code").await?;
        if !output.status.success() { return Ok(GateResult::fail("dead_code", "cargo check failed – fix compilation first")); }
        let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
        if stderr.contains("dead_code") || stderr.contains("unused") {
            Ok(GateResult::fail_with_details("dead_code", "dead code or unused imports found", stderr))
        } else { Ok(GateResult::pass("dead_code")) }
    }
}
```

```rust
// coverage.rs
use crate::error::PilotError;
use crate::gates::helpers::{is_command_not_found, run_gate_command};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use regex::Regex;
use std::sync::LazyLock;
static COVERAGE_PCT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+\.?\d*)%\s*coverage").unwrap());

pub struct CoverageGate { pub min_coverage: f64 }
#[async_trait]
impl Gate for CoverageGate {
    fn name(&self) -> &str { "coverage" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_gate_command("cargo", &["llvm-cov", "--summary-only"], &ctx.worktree_dir, ctx.timeout_secs, "coverage").await;
        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                match COVERAGE_PCT_RE.captures(&stdout).and_then(|c| c.get(1)).and_then(|m| m.as_str().parse::<f64>().ok()) {
                    Some(pct) if pct >= self.min_coverage => Ok(GateResult::pass("coverage")),
                    Some(pct) => Ok(GateResult::fail_with_details("coverage", format!("coverage {pct:.0}% < {}%", self.min_coverage), stdout.to_string())),
                    None => Ok(GateResult::fail("coverage", "could not parse coverage output")),
                }
            }
            Ok(out) => {
                let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                if is_command_not_found(&stdout, &stderr) {
                    Err(PilotError::Gate { gate: "coverage".into(), message: "cargo-llvm-cov not installed".into() })
                } else { Ok(GateResult::fail("coverage", "cargo llvm-cov failed")) }
            }
            Err(e) => Err(e),
        }
    }
}
```

```rust
// mutation.rs — same pattern as coverage but with cargo mutants
use crate::error::PilotError;
use crate::gates::helpers::{is_command_not_found, run_gate_command};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use regex::Regex;
use std::sync::LazyLock;
static MUTATION_PCT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\((\d+\.?\d*)%\)").unwrap());

pub struct MutationGate { pub min_kill_rate: f64 }
#[async_trait]
impl Gate for MutationGate {
    fn name(&self) -> &str { "mutation" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_gate_command("cargo", &["mutants", "--no-times"], &ctx.worktree_dir, ctx.timeout_secs, "mutation").await;
        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                match MUTATION_PCT_RE.captures(&stdout).and_then(|c| c.get(1)).and_then(|m| m.as_str().parse::<f64>().ok()) {
                    Some(rate) if rate >= self.min_kill_rate => Ok(GateResult::pass("mutation")),
                    Some(rate) => Ok(GateResult::fail_with_details("mutation", format!("kill rate {rate:.0}% < {}%", self.min_kill_rate), stdout.to_string())),
                    None => Ok(GateResult::fail("mutation", "could not parse mutation output")),
                }
            }
            Ok(out) => {
                let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                if is_command_not_found(&stdout, &stderr) {
                    Err(PilotError::Gate { gate: "mutation".into(), message: "cargo-mutants not installed".into() })
                } else { Ok(GateResult::fail("mutation", "cargo mutants failed")) }
            }
            Err(e) => Err(e),
        }
    }
}
```

```rust
// workspace_check.rs
use crate::error::PilotError;
use crate::gates::helpers::{run_gate_command, strip_ansi};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct WorkspaceCheckGate;
#[async_trait]
impl Gate for WorkspaceCheckGate {
    fn name(&self) -> &str { "workspace_check" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_gate_command("cargo", &["check", "--workspace"], &ctx.worktree_dir, ctx.timeout_secs, "workspace_check").await?;
        if output.status.success() { Ok(GateResult::pass("workspace_check")) }
        else { Ok(GateResult::fail_with_details("workspace_check", "workspace check failed", strip_ansi(&String::from_utf8_lossy(&output.stderr)))) }
    }
}
```

```rust
// audit.rs
use crate::error::PilotError;
use crate::gates::helpers::{is_command_not_found, run_gate_command};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct AuditGate;
#[async_trait]
impl Gate for AuditGate {
    fn name(&self) -> &str { "audit" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_gate_command("cargo", &["audit"], &ctx.worktree_dir, ctx.timeout_secs, "audit").await;
        match output {
            Ok(out) if out.status.success() => Ok(GateResult::pass("audit")),
            Ok(out) => {
                let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
                if is_command_not_found(&stdout, &stderr) {
                    Err(PilotError::Gate { gate: "audit".into(), message: "cargo-audit not installed".into() })
                } else { Ok(GateResult::fail_with_details("audit", "vulnerabilities found", format!("{stdout}{stderr}"))) }
            }
            Err(e) => Err(e),
        }
    }
}
```

## src/gates/banned_pattern.rs

**Fix #7 from critique**: Diff-based scanning contract is documented. **Fix #8 from critique**: Uses `ctx.changed_files` instead of making its own git call. **Fix #12 from critique**: `strip_string_literals` now handles raw strings (`r"..."`, `r#"..."#`) and byte strings (`b"..."`, `br"..."`).

```rust
//! Banned pattern gate.
//!
//! # Scanning Contract (Fix #7)
//!
//! This gate checks **changed files only** (via git diff --name-only),
//! not all files in the worktree. Pre-existing violations in unchanged
//! files will NOT be detected. If a full scan is needed, use the
//! `--full-scan` option (future work) or run the gate on a clean
//! checkout where all files are "changed."
//!
//! The changed file list is pre-computed by the pipeline and passed
//! via `GateContext::changed_files` to avoid redundant git calls.

use crate::domain_types::{BannedPattern, default_banned_patterns};
use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct BannedPatternGate { patterns: Vec<BannedPattern> }

impl BannedPatternGate {
    pub fn new(patterns: Vec<BannedPattern>) -> Self {
        Self { patterns: if patterns.is_empty() { default_banned_patterns() } else { patterns } }
    }
    pub fn with_defaults() -> Self { Self::new(default_banned_patterns()) }
}
impl Default for BannedPatternGate { fn default() -> Self { Self::with_defaults() } }

#[async_trait]
impl Gate for BannedPatternGate {
    fn name(&self) -> &str { "banned_patterns" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let changed_files = ctx.changed_files.clone();
        let use_diff = !changed_files.is_empty();
        let dir = ctx.worktree_dir.clone();
        let patterns = self.patterns.clone();

        let result = tokio::task::spawn_blocking(move || {
            let mut violations = Vec::new();

            if use_diff {
                // Diff-based scan: only check changed .rs files (Fix #7)
                for rel_path in &changed_files {
                    if !rel_path.ends_with(".rs") { continue; }
                    let path = dir.join(rel_path);
                    let path_str = path.to_string_lossy();
                    if path_str.contains("/tests/") || path_str.contains("\\tests\\") { continue; }
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        check_content_for_violations(&content, rel_path, &patterns, &mut violations);
                    }
                }
            } else {
                // Full tree walk fallback (e.g. first commit)
                let walker = ignore::WalkBuilder::new(&dir).hidden(false).build();
                for entry in walker.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(false, |e| e == "rs") {
                        let path_str = path.to_string_lossy();
                        if path_str.contains("/tests/") || path_str.contains("\\tests\\") { continue; }
                        let rel = path.strip_prefix(&dir).unwrap_or(path);
                        let rel_str = rel.to_string_lossy().to_string();
                        if let Ok(content) = std::fs::read_to_string(path) {
                            check_content_for_violations(&content, &rel_str, &patterns, &mut violations);
                        }
                    }
                }
            }

            if violations.is_empty() { GateResult::pass("banned_patterns") }
            else { GateResult::fail_with_details("banned_patterns", format!("{} banned pattern(s) found", violations.len()), violations.join("\n")) }
        }).await.map_err(|e| PilotError::Gate { gate: "banned_patterns".into(), message: format!("spawn_blocking: {e}") })?;
        Ok(result)
    }
}

fn check_content_for_violations(
    content: &str, rel_path: &str, patterns: &[BannedPattern], violations: &mut Vec<String>,
) {
    for (i, line) in content.lines().enumerate() {
        if line.trim().starts_with("//") { continue; }
        let check_line = strip_string_literals(line);
        for pattern in patterns {
            if check_line.contains(&pattern.pattern) {
                violations.push(format!("{}:{}: {}", rel_path, i + 1, pattern.description));
            }
        }
    }
}

/// Strip string literals (regular, raw, byte, byte-raw) to reduce
/// false positives on patterns inside string content.
///
/// Handles:
/// - Regular strings: `"..."` with `\"` escapes
/// - Raw strings: `r"..."`, `r#"..."#`, `r##"..."##`
/// - Byte strings: `b"..."` with `\"` escapes
/// - Byte raw strings: `br"..."`, `br#"..."#`
/// - Character literals: `'...'`
/// - Line comments: `// ...`
///
/// Does NOT handle: `r#keyword` (raw identifiers), doc comments
/// containing code, or macro invocations with custom delimiters.
fn strip_string_literals(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();

    // Line comment — skip rest of line
    'outer: while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') { break; }

        // Byte string: b"..." or br"..."
        if c == 'b' {
            let next = chars.peek().copied();
            if next == Some('"') {
                chars.next(); // consume opening "
                while let Some(nc) = chars.next() {
                    if nc == '\\' { chars.next(); continue; }
                    if nc == '"' { continue 'outer; }
                }
                continue;
            }
            if next == Some('r') {
                chars.next(); // consume 'r'
                consume_raw_string_inner(&mut chars);
                continue;
            }
            result.push(c);
            continue;
        }

        // Raw string: r"...", r#"..."#, etc.
        if c == 'r' {
            let next = chars.peek().copied();
            if next == Some('"') || next == Some('#') {
                consume_raw_string_inner(&mut chars);
                continue;
            }
            result.push(c);
            continue;
        }

        // Regular string
        if c == '"' {
            while let Some(nc) = chars.next() {
                if nc == '\\' { chars.next(); continue; }
                if nc == '"' { continue 'outer; }
            }
            continue;
        }

        // Character literal
        if c == '\'' {
            while let Some(nc) = chars.next() {
                if nc == '\\' { chars.next(); continue; }
                if nc == '\'' { continue 'outer; }
            }
            continue;
        }

        result.push(c);
    }
    result
}

/// Consume a raw string body after the opening `r` (and for byte raw,
/// after `br`). Handles `r"..."`, `r#"..."#`, `r##"..."##`, etc.
fn consume_raw_string_inner(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    let mut hash_count = 0usize;
    while chars.peek() == Some(&'#') { chars.next(); hash_count += 1; }
    if chars.peek() != Some(&'"') { return; } // not actually a raw string
    chars.next(); // consume opening "

    let mut prev_quote = false;
    let mut hash_matched = 0;
    loop {
        match chars.next() {
            None => break,
            Some('"') => { prev_quote = true; hash_matched = 0; }
            Some('#') if prev_quote => {
                hash_matched += 1;
                if hash_matched == hash_count { break; }
            }
            Some(_) => { prev_quote = false; hash_matched = 0; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_regular_string() {
        assert_eq!(strip_string_literals(r#"let x = "used as input"; let y = foo as Bar;"#), "let x = ; let y = foo as Bar;");
    }

    #[test]
    fn test_strip_raw_string() {
        assert_eq!(strip_string_literals(r#"let s = r"contains { braces }"; let y = x as i32;"#), "let s = ; let y = x as i32;");
    }

    #[test]
    fn test_strip_raw_string_hash() {
        assert_eq!(strip_string_literals(r#"let s = r#"contains { braces }"#; let y = x as i32;"#), "let s = ; let y = x as i32;");
    }

    #[test]
    fn test_strip_raw_string_double_hash() {
        assert_eq!(strip_string_literals(r#"let s = r##"contains { braces }"##; let y = x as i32;"#), "let s = ; let y = x as i32;");
    }

    #[test]
    fn test_strip_byte_string() {
        assert_eq!(strip_string_literals(r#"let b = b"hello {"; let y = x as i32;"#), "let b = ; let y = x as i32;");
    }

    #[test]
    fn test_strip_byte_raw_string() {
        assert_eq!(strip_string_literals(r#"let b = br#"contains { braces }"#; let y = x as i32;"#), "let b = ; let y = x as i32;");
    }

    #[test]
    fn test_strip_escaped_quote() {
        assert_eq!(strip_string_literals(r#"let s = "he said \"hi\""; let y = x as i32;"#), "let s = ; let y = x as i32;");
    }

    #[test]
    fn test_strip_no_strings() {
        assert_eq!(strip_string_literals("let y = foo as Bar;"), "let y = foo as Bar;");
    }

    #[test]
    fn test_strip_line_comment() {
        assert_eq!(strip_string_literals("let x = 1; // comment with \"string\""), "let x = 1; ");
    }

    #[test]
    fn test_banned_pattern_in_raw_string_no_false_positive() {
        // `unwrap()` inside a raw string should NOT be flagged
        let line = r#"let msg = r"call .unwrap() here"; let y = z;"#;
        let stripped = strip_string_literals(line);
        assert!(!stripped.contains("unwrap"), "raw string content should be stripped: got '{stripped}'");
    }

    #[test]
    fn test_comments_skipped() {
        let content = "// todo!() should not be flagged\nfn main() {}\n";
        let patterns = default_banned_patterns();
        let mut violations = Vec::new();
        check_content_for_violations(content, "test.rs", &patterns, &mut violations);
        assert!(violations.is_empty());
    }
}
```

## src/gates/architecture.rs

**Fix #7 from critique**: Contract documented. **Fix #8 from critique**: Uses `ctx.changed_files`.

```rust
//! Architecture dependency gate.
//!
//! # Scanning Contract (Fix #7)
//!
//! Checks **changed Cargo.toml files only** (via pre-computed
//! changed_files in GateContext). Pre-existing violations in
//! unchanged files are NOT detected.

use crate::domain_types::{DependencyRule, default_architecture_rules};
use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct ArchitectureGate { rules: Vec<DependencyRule> }
impl ArchitectureGate {
    pub fn new(rules: Vec<DependencyRule>) -> Self { Self { rules: if rules.is_empty() { default_architecture_rules() } else { rules } } }
    pub fn with_default_rules() -> Self { Self::new(default_architecture_rules()) }
}
impl Default for ArchitectureGate { fn default() -> Self { Self::with_default_rules() } }

#[async_trait]
impl Gate for ArchitectureGate {
    fn name(&self) -> &str { "architecture" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let changed_files = ctx.changed_files.clone();
        let use_diff = !changed_files.is_empty();
        let dir = ctx.worktree_dir.clone();
        let rules = self.rules.clone();

        let result = tokio::task::spawn_blocking(move || {
            let mut violations = Vec::new();
            if use_diff {
                for rel_path in &changed_files {
                    if !rel_path.ends_with("Cargo.toml") { continue; }
                    let path = dir.join(rel_path);
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        check_cargo_toml(&content, rel_path, &rules, &mut violations);
                    }
                }
            } else {
                let walker = ignore::WalkBuilder::new(&dir).hidden(false).build();
                for entry in walker.flatten() {
                    let path = entry.path();
                    if path.file_name().map_or(false, |n| n == "Cargo.toml") {
                        let rel = path.strip_prefix(&dir).unwrap_or(path);
                        if let Ok(content) = std::fs::read_to_string(path) {
                            check_cargo_toml(&content, &rel.to_string_lossy(), &rules, &mut violations);
                        }
                    }
                }
            }
            if violations.is_empty() { GateResult::pass("architecture") }
            else { GateResult::fail_with_details("architecture", format!("{} violation(s)", violations.len()), violations.join("\n")) }
        }).await.map_err(|e| PilotError::Gate { gate: "architecture".into(), message: format!("spawn_blocking: {e}") })?;
        Ok(result)
    }
}

fn check_cargo_toml(content: &str, rel_path: &str, rules: &[DependencyRule], violations: &mut Vec<String>) {
    if let Some(crate_name) = extract_crate_name_toml(content) {
        let deps = extract_dependencies_toml(content);
        for rule in rules {
            if crate_name == rule.from_crate && deps.contains(&rule.forbidden_dep) {
                violations.push(format!("{}: {} depends on {} – {}", rel_path, rule.from_crate, rule.forbidden_dep, rule.reason));
            }
        }
    }
}

fn extract_crate_name_toml(content: &str) -> Option<String> {
    toml::from_str::<toml::Value>(content).ok()?.get("package")?.get("name")?.as_str().map(|s| s.to_string())
}

fn extract_dependencies_toml(content: &str) -> Vec<String> {
    let value = match toml::from_str::<toml::Value>(content) { Ok(v) => v, Err(_) => return Vec::new() };
    value.get("dependencies").and_then(|v| v.as_table()).map(|t| t.keys().cloned().collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_extract_deps() {
        let cargo = r#"[package]\nname = "glyim-frontend"\nversion = "0.1.0"\n\n[dependencies]\nglyim-ir = { path = "../ir" }\nserde = "1"\n\n[dev-dependencies]\ntempfile = "3"\n"#;
        let deps = extract_dependencies_toml(cargo);
        assert!(deps.contains(&"glyim-ir".to_string()));
        assert!(!deps.contains(&"tempfile".to_string()));
    }
}
```

## src/gates/contracts.rs

```rust
use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct ContractGate;
#[async_trait]
impl Gate for ContractGate {
    fn name(&self) -> &str { "contracts" }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let contracts_path = ctx.project_root.join("CONTRACTS_LOCKED.md");
        let locked_names = if contracts_path.exists() {
            let content = tokio::fs::read_to_string(&contracts_path).await
                .map_err(|e| PilotError::Gate { gate: "contracts".into(), message: format!("read failed: {e}") })?;
            extract_locked_names(&content)
        } else { return Ok(GateResult::pass_with_note("contracts", "no CONTRACTS_LOCKED.md found")); };

        if locked_names.is_empty() { return Ok(GateResult::pass("contracts")); }

        let diff = crate::git_ops::diff_main(&ctx.worktree_dir, &ctx.default_branch, ctx.timeout_secs).await?;
        if diff.is_empty() { return Ok(GateResult::pass("contracts")); }

        let mut violations = Vec::new();
        for line in diff.lines() {
            if line.starts_with('-') && !line.starts_with("---") {
                for name in &locked_names {
                    if line.contains(name.as_str()) {
                        violations.push(format!("locked interface '{}' in removed line: {}", name, line.trim_start_matches('-').trim()));
                    }
                }
            }
        }
        if violations.is_empty() { Ok(GateResult::pass("contracts")) }
        else { Ok(GateResult::fail_with_details("contracts", format!("{} violation(s)", violations.len()), violations.join("\n"))) }
    }
}

fn extract_locked_names(content: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_code = false;
    for line in content.lines() {
        if line.trim().starts_with("```") { in_code = !in_code; continue; }
        if !in_code { continue; }
        if let Some(name) = extract_pub_name(line.trim()) { names.push(name); }
    }
    names
}

fn extract_pub_name(line: &str) -> Option<String> {
    let after_pub = if let Some(r) = line.strip_prefix("pub async fn ") { ("fn", r) }
        else if let Some(r) = line.strip_prefix("pub const fn ") { ("fn", r) }
        else if let Some(r) = line.strip_prefix("pub(crate) async fn ") { ("fn", r) }
        else if let Some(r) = line.strip_prefix("pub(crate) fn ") { ("fn", r) }
        else if let Some(r) = line.strip_prefix("pub fn ") { ("fn", r) }
        else if let Some(r) = line.strip_prefix("pub struct ") { ("struct", r) }
        else if let Some(r) = line.strip_prefix("pub enum ") { ("enum", r) }
        else if let Some(r) = line.strip_prefix("pub trait ") { ("trait", r) }
        else { return None };

    match after_pub.0 {
        "fn" => after_pub.1.split('(').next().map(|s| s.trim().to_string()),
        "struct" | "enum" => after_pub.1.split(|c: char| c == '<' || c == '{' || c == ' ' || c == ';').next().map(|s| s.trim().to_string()),
        "trait" => after_pub.1.split(|c: char| c == '<' || c == '{' || c == ':').next().map(|s| s.trim().to_string()),
        _ => None,
    }
}
```

## src/gates/self_review.rs

```rust
pub fn build_review_prompt(diff: &str, commit_log: &str) -> String {
    format!(
r#"## Self-Review Required

### Commit History
```
{commit_log}
```

### Full Diff
```diff
{diff}
```

### Review Checklist
1. Edge cases handled?
2. No unnecessary allocations?
3. All error paths covered?
4. Public interfaces consistent?
5. Tests cover happy AND failure paths?
6. No dead code?
7. Public items documented?
8. Naming clear and consistent?

Respond with your review, then either fix issues or emit `::APPROVED`.
"#
    )
}
```

## src/gates/commit_pipeline.rs

**Fix #8 from critique**: Changed files computed once and passed to all gates via GateContext.

```rust
use crate::config::types::ResolvedCommitGates;
use crate::domain_types::{BannedPattern, DependencyRule};
use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{
    Gate, GateResult, PipelineResult,
    architecture::ArchitectureGate, banned_pattern::BannedPatternGate,
    check::CheckGate, clippy::ClippyGate, contracts::ContractGate,
    fmt_check::FmtCheckGate, test_gate::TestGate,
};
use std::sync::Arc;
use std::time::Instant;

pub async fn run_commit_pipeline(
    ctx: &GateContext,
    config: &ResolvedCommitGates,
    banned_patterns: Vec<BannedPattern>,
    architecture_rules: Vec<DependencyRule>,
) -> Result<PipelineResult, PilotError> {
    let mut gates: Vec<Arc<dyn Gate>> = Vec::new();
    if config.fmt { gates.push(Arc::new(FmtCheckGate)); }
    if config.check { gates.push(Arc::new(CheckGate)); }
    if config.clippy { gates.push(Arc::new(ClippyGate)); }
    if config.test { gates.push(Arc::new(TestGate)); }
    if config.banned_patterns { gates.push(Arc::new(BannedPatternGate::new(banned_patterns))); }
    if config.architecture { gates.push(Arc::new(ArchitectureGate::new(architecture_rules))); }
    if config.contracts { gates.push(Arc::new(ContractGate)); }

    let mut results = Vec::new();
    for gate in &gates {
        let start = Instant::now();
        let result = gate.run(ctx).await?;
        tracing::info!(gate = gate.name(), elapsed = ?start.elapsed(), passed = result.passed, "commit gate completed");
        let passed = result.passed;
        results.push(result);
        if !passed { break; }
    }
    Ok(PipelineResult::from_gates(results))
}
```

## src/gates/done_pipeline.rs

```rust
use crate::config::types::ResolvedDoneGates;
use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult, PipelineResult, audit::AuditGate, coverage::CoverageGate, dead_code::DeadCodeGate, mutation::MutationGate, workspace_check::WorkspaceCheckGate};
use std::sync::Arc;
use std::time::Instant;

pub async fn run_done_pipeline(ctx: &GateContext, config: &ResolvedDoneGates) -> Result<PipelineResult, PilotError> {
    let mut gates: Vec<Arc<dyn Gate>> = Vec::new();
    if config.dead_code { gates.push(Arc::new(DeadCodeGate)); }
    if config.coverage { gates.push(Arc::new(CoverageGate { min_coverage: config.coverage_min })); }
    if config.mutation { gates.push(Arc::new(MutationGate { min_kill_rate: config.mutation_kill_rate })); }
    if config.workspace_check { gates.push(Arc::new(WorkspaceCheckGate)); }
    if config.audit { gates.push(Arc::new(AuditGate)); }

    if gates.is_empty() { return Ok(PipelineResult::from_gates(vec![GateResult::pass("done_pipeline")])); }

    let mut results = Vec::new();
    for gate in &gates {
        let result = gate.run(ctx).await?;
        let passed = result.passed;
        results.push(result);
        if !passed { break; }
    }
    Ok(PipelineResult::from_gates(results))
}
```

## src/commit/mod.rs

```rust
pub mod engine;
pub use engine::{CommitEngine, CommitDecision, CommitContext};
```

## src/commit/engine.rs

```rust
use crate::config::types::ResolvedCommitGates;
use crate::domain_types::{BannedPattern, DependencyRule};
use crate::error::PilotError;
use crate::gates::commit_pipeline;
use crate::gates::fmt_fix;
use crate::gates::types::GateContext;
use crate::git_ops::{commit_all, emergency_wip_commit};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum CommitDecision {
    Committed { message: String, new_fix_round: u32 },
    GateFailed { new_fix_round: u32, feedback: String },
    Escalated { new_fix_round: u32, feedback: String },
}

impl CommitDecision {
    pub fn new_fix_round(&self) -> u32 {
        match self {
            Self::Committed { new_fix_round, .. } => *new_fix_round,
            Self::GateFailed { new_fix_round, .. } => *new_fix_round,
            Self::Escalated { new_fix_round, .. } => *new_fix_round,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommitContext {
    pub worktree_dir: PathBuf,
    pub project_root: PathBuf,
    pub stream_id: String,
    pub commit_message: String,
    pub current_fix_round: u32,
    pub timeout_secs: u64,
    pub default_branch: String,
    pub branch_version: String,
    pub changed_files: Vec<String>,
}

pub struct CommitEngine {
    gate_config: ResolvedCommitGates,
    max_fix_rounds: u32,
    banned_patterns: Vec<BannedPattern>,
    architecture_rules: Vec<DependencyRule>,
}

impl CommitEngine {
    pub fn new(
        gate_config: ResolvedCommitGates, max_fix_rounds: u32,
        banned_patterns: Vec<BannedPattern>, architecture_rules: Vec<DependencyRule>,
    ) -> Self {
        Self { gate_config, max_fix_rounds, banned_patterns, architecture_rules }
    }

    pub async fn evaluate_commit(&self, ctx: &CommitContext) -> Result<CommitDecision, PilotError> {
        let gate_ctx = GateContext::new(
            ctx.worktree_dir.clone(), ctx.project_root.clone(),
            ctx.default_branch.clone(), ctx.branch_version.clone(),
            ctx.timeout_secs, ctx.changed_files.clone(),
        );

        let pipeline_result = commit_pipeline::run_commit_pipeline(
            &gate_ctx, &self.gate_config,
            self.banned_patterns.clone(), self.architecture_rules.clone(),
        ).await?;

        if pipeline_result.passed {
            commit_all(&ctx.worktree_dir, &ctx.stream_id, &ctx.commit_message, ctx.timeout_secs).await?;
            Ok(CommitDecision::Committed { message: ctx.commit_message.clone(), new_fix_round: 0 })
        } else {
            // If the fmt gate failed, try auto-fixing before reporting failure
            let fmt_failed = pipeline_result.gates.iter().any(|g| g.gate_name == "fmt" && !g.passed);

            if fmt_failed {
                tracing::info!("fmt gate failed — attempting auto-fix before reporting");
                let fix_result = fmt_fix::run_fmt_fix(&gate_ctx).await?;
                if fix_result.passed {
                    // Re-run the commit pipeline after auto-fix
                    // Re-fetch changed files since fmt may have modified them
                    let updated_changed = crate::git_ops::diff_name_only(
                        &ctx.worktree_dir, &ctx.default_branch, ctx.timeout_secs,
                    ).await.unwrap_or_default()
                        .lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();

                    let retry_ctx = GateContext::new(
                        ctx.worktree_dir.clone(), ctx.project_root.clone(),
                        ctx.default_branch.clone(), ctx.branch_version.clone(),
                        ctx.timeout_secs, updated_changed,
                    );
                    let retry_result = commit_pipeline::run_commit_pipeline(
                        &retry_ctx, &self.gate_config,
                        self.banned_patterns.clone(), self.architecture_rules.clone(),
                    ).await?;
                    if retry_result.passed {
                        let fix_msg = format!("{} (fmt auto-fixed)", ctx.commit_message);
                        commit_all(&ctx.worktree_dir, &ctx.stream_id, &fix_msg, ctx.timeout_secs).await?;
                        return Ok(CommitDecision::Committed { message: fix_msg, new_fix_round: 0 });
                    }
                    let feedback = retry_result.failure_message();
                    return self.escalate_or_retry(ctx, ctx.current_fix_round + 1, &feedback);
                }
            }

            let feedback = pipeline_result.failure_message();
            self.escalate_or_retry(ctx, ctx.current_fix_round + 1, &feedback)
        }
    }

    fn escalate_or_retry(
        &self, ctx: &CommitContext, new_fix_round: u32, feedback: &str,
    ) -> Result<CommitDecision, PilotError> {
        if new_fix_round > self.max_fix_rounds {
            Ok(CommitDecision::Escalated { new_fix_round, feedback: feedback.to_string() })
        } else {
            Ok(CommitDecision::GateFailed { new_fix_round, feedback: feedback.to_string() })
        }
    }

    pub async fn emergency_commit(&self, ctx: &CommitContext) -> Result<(), PilotError> {
        emergency_wip_commit(&ctx.worktree_dir, &ctx.stream_id, ctx.timeout_secs).await
    }
}
```

## src/session/mod.rs

```rust
pub mod state;
pub mod machine;
pub mod persistence;

pub use state::{SessionState, StreamStatus, GlobalState};
pub use machine::TransitionValidator;
pub use persistence::StatePersistence;
```

## src/session/state.rs

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StreamStatus {
    Init, Seeding, Waiting, Streaming, Executing, Feedback,
    Committing, Committed, Verifying, Reviewing, Complete, Error, Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: String,
    pub stream_id: String,
    pub provider_id: String,
    pub tab_id: Option<u64>,
    pub status: StreamStatus,
    pub turn: u32,
    pub fix_round: u32,
    pub commits: u32,
    pub worktree_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub error_message: Option<String>,
    pub provider_cooldown_until: Option<DateTime<Utc>>,
}

impl SessionState {
    pub fn new(stream_id: String, provider_id: String, worktree_path: String) -> Self {
        let now = Utc::now();
        Self {
            session_id: uuid::Uuid::new_v4().to_string(), stream_id, provider_id,
            tab_id: None, status: StreamStatus::Init, turn: 0, fix_round: 0, commits: 0,
            worktree_path, created_at: now, updated_at: now, last_activity: now,
            error_message: None, provider_cooldown_until: None,
        }
    }

    pub(crate) fn transition(&mut self, new_status: StreamStatus) {
        let now = Utc::now();
        self.status = new_status;
        self.updated_at = now;
        self.last_activity = now;
    }

    pub(crate) fn record_commit(&mut self) { self.commits += 1; self.fix_round = 0; self.last_activity = Utc::now(); }
    pub(crate) fn record_turn(&mut self) { self.turn += 1; self.last_activity = Utc::now(); }
    pub(crate) fn set_provider_cooldown(&mut self, until: DateTime<Utc>) { self.provider_cooldown_until = Some(until); }
    pub fn is_provider_in_cooldown(&self) -> bool { self.provider_cooldown_until.map_or(false, |until| Utc::now() < until) }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalState {
    pub sessions: HashMap<String, SessionState>,
    pub version: String,
}

impl GlobalState {
    pub fn new() -> Self { Self { sessions: HashMap::new(), version: env!("CARGO_PKG_VERSION").to_string() } }
}
impl Default for GlobalState { fn default() -> Self { Self::new() } }
```

## src/session/machine.rs

```rust
use crate::error::PilotError;
use super::state::{SessionState, StreamStatus};

const VALID_TRANSITIONS: &[(StreamStatus, StreamStatus)] = &[
    (StreamStatus::Init, StreamStatus::Seeding),
    (StreamStatus::Init, StreamStatus::Error),
    (StreamStatus::Seeding, StreamStatus::Waiting),
    (StreamStatus::Seeding, StreamStatus::Error),
    (StreamStatus::Waiting, StreamStatus::Streaming),
    (StreamStatus::Waiting, StreamStatus::Executing),
    (StreamStatus::Waiting, StreamStatus::Paused),
    (StreamStatus::Waiting, StreamStatus::Error),
    (StreamStatus::Streaming, StreamStatus::Executing),
    (StreamStatus::Streaming, StreamStatus::Error),
    (StreamStatus::Executing, StreamStatus::Feedback),
    (StreamStatus::Executing, StreamStatus::Error),
    (StreamStatus::Executing, StreamStatus::Committing),
    (StreamStatus::Feedback, StreamStatus::Waiting),
    (StreamStatus::Feedback, StreamStatus::Executing),
    (StreamStatus::Feedback, StreamStatus::Committing),
    (StreamStatus::Committing, StreamStatus::Committed),
    (StreamStatus::Committing, StreamStatus::Feedback),
    (StreamStatus::Committed, StreamStatus::Waiting),
    (StreamStatus::Committed, StreamStatus::Verifying),
    (StreamStatus::Verifying, StreamStatus::Reviewing),
    (StreamStatus::Verifying, StreamStatus::Feedback),
    (StreamStatus::Reviewing, StreamStatus::Complete),
    (StreamStatus::Reviewing, StreamStatus::Feedback),
    (StreamStatus::Error, StreamStatus::Seeding),
    (StreamStatus::Error, StreamStatus::Paused),
    (StreamStatus::Paused, StreamStatus::Seeding),
];

pub struct TransitionValidator;

impl TransitionValidator {
    pub fn validate(session: &SessionState, new_status: StreamStatus) -> Result<(), PilotError> {
        if session.status == new_status { return Ok(()); }
        if VALID_TRANSITIONS.iter().any(|(from, to)| from == &session.status && to == &new_status) {
            Ok(())
        } else {
            Err(PilotError::Session(format!("invalid state transition: {:?} → {:?} (session {})", session.status, new_status, session.stream_id)))
        }
    }

    pub fn transition(session: &mut SessionState, new_status: StreamStatus) -> Result<(), PilotError> {
        Self::validate(session, new_status)?;
        session.transition(new_status);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_session() -> SessionState { SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into()) }

    #[test]
    fn test_valid_transition() { assert!(TransitionValidator::validate(&make_session(), StreamStatus::Seeding).is_ok()); }
    #[test]
    fn test_invalid_transition() { assert!(TransitionValidator::validate(&make_session(), StreamStatus::Complete).is_err()); }
    #[test]
    fn test_same_state_noop() { assert!(TransitionValidator::validate(&make_session(), StreamStatus::Init).is_ok()); }
}
```

## src/session/persistence.rs

**Fix #6 from critique**: The old `DebouncedPersistence` was fake — every mutation called `flush()` immediately and the background task did nothing. Now renamed to `StatePersistence` with honest immediate saves. The `start_flush_task` dead code is deleted. **Fix #5**: No `lock()` method exposed; all access through specific query/mutation methods.

```rust
//! State persistence with immediate saves.
//!
//! Every mutation persists to disk immediately. There is no debouncing
//! because every state transition is critical — losing a transition
//! means the session is in an inconsistent state. The old
//! "DebouncedPersistence" was a facade that claimed debouncing but
//! actually flushed on every call; this version is honest about it.

use crate::error::PilotError;
use super::state::{GlobalState, SessionState, StreamStatus};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

const STATE_FILE: &str = ".glyim-pilot-state.json";

struct Inner {
    path: PathBuf,
    state: GlobalState,
}

impl Inner {
    async fn load(project_root: &Path) -> Result<Self, PilotError> {
        let path = project_root.join(STATE_FILE);
        let state = if path.exists() {
            let content = tokio::fs::read_to_string(&path).await
                .map_err(|e| PilotError::Session(format!("failed to read state: {e}")))?;
            serde_json::from_str(&content)
                .map_err(|e| PilotError::Session(format!("failed to parse state: {e}")))?
        } else {
            GlobalState::new()
        };
        tracing::info!(path = %path.display(), sessions = state.sessions.len(), "loaded persistence");
        Ok(Self { path, state })
    }

    async fn save(&self) -> Result<(), PilotError> {
        let content = serde_json::to_string(&self.state)
            .map_err(|e| PilotError::Session(format!("serialization failed: {e}")))?;
        let tmp_path = PathBuf::from(format!("{}.tmp", self.path.display()));
        if let Err(e) = tokio::fs::write(&tmp_path, &content).await {
            tracing::warn!(path = %tmp_path.display(), error = %e, "save: temp write failed");
            return Err(PilotError::Session(format!("temp write failed: {e}")));
        }
        if let Err(e) = tokio::fs::rename(&tmp_path, &self.path).await {
            tracing::warn!(path = %self.path.display(), error = %e, "save: rename failed");
            let _ = std::fs::remove_file(&tmp_path);
            return Err(PilotError::Session(format!("rename failed: {e}")));
        }
        Ok(())
    }

    fn add_session(&mut self, session: SessionState) {
        self.state.sessions.insert(session.stream_id.clone(), session);
    }

    fn get_session(&self, stream_id: &str) -> Option<&SessionState> {
        self.state.sessions.get(stream_id)
    }

    fn all_sessions(&self) -> Vec<&SessionState> {
        self.state.sessions.values().collect()
    }

    fn remove_session(&mut self, stream_id: &str) {
        self.state.sessions.remove(stream_id);
    }

    fn session_count(&self) -> usize { self.state.sessions.len() }
}

/// Thread-safe state persistence with immediate saves.
/// No `lock()` exposure — all access through specific methods.
pub struct StatePersistence {
    inner: Mutex<Inner>,
}

impl StatePersistence {
    pub async fn load(project_root: &Path) -> Result<Self, PilotError> {
        let inner = Inner::load(project_root).await?;
        Ok(Self { inner: Mutex::new(inner) })
    }

    pub async fn add_session(&self, session: SessionState) -> Result<(), PilotError> {
        let mut p = self.inner.lock().await;
        p.add_session(session);
        p.save().await
    }

    pub async fn try_update_session<F>(&self, stream_id: &str, f: F) -> Result<(), PilotError>
    where F: FnOnce(&mut SessionState) -> Result<(), PilotError> {
        let mut p = self.inner.lock().await;
        let session = p.state.sessions.get_mut(stream_id)
            .ok_or_else(|| PilotError::Session(format!("session {stream_id} not found")))?;
        let backup = session.clone();
        if let Err(e) = f(session) {
            *p.state.sessions.get_mut(stream_id).unwrap() = backup;
            return Err(e);
        }
        if let Err(e) = p.save().await {
            // Revert in-memory state on save failure
            if let Some(s) = p.state.sessions.get_mut(stream_id) { *s = backup; }
            tracing::warn!(error = %e, "flush after update failed — reverted in-memory state");
            return Err(e);
        }
        Ok(())
    }

    pub async fn remove_session(&self, stream_id: &str) -> Result<(), PilotError> {
        let mut p = self.inner.lock().await;
        p.remove_session(stream_id);
        p.save().await
    }

    pub async fn get_session(&self, stream_id: &str) -> Option<SessionState> {
        self.inner.lock().await.get_session(stream_id).cloned()
    }

    pub async fn get_worktree_path(&self, stream_id: &str) -> Option<String> {
        self.inner.lock().await.get_session(stream_id).map(|s| s.worktree_path.clone())
    }

    pub async fn get_stream_id(&self, session_id: &str) -> Option<String> {
        self.inner.lock().await.all_sessions().iter()
            .find(|s| s.session_id == session_id).map(|s| s.stream_id.clone())
    }

    pub async fn get_fix_round(&self, stream_id: &str) -> u32 {
        self.inner.lock().await.get_session(stream_id).map(|s| s.fix_round).unwrap_or(0)
    }

    pub async fn all_sessions(&self) -> Vec<SessionState> {
        self.inner.lock().await.all_sessions().into_iter().cloned().collect()
    }

    pub async fn session_count(&self) -> usize {
        self.inner.lock().await.session_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::machine::TransitionValidator;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, StatePersistence) {
        let dir = tempfile::tempdir().unwrap();
        let p = StatePersistence::load(dir.path()).await.unwrap();
        (dir, p)
    }

    #[tokio::test]
    async fn test_add_and_persist() {
        let dir = tempfile::tempdir().unwrap();
        let p = StatePersistence::load(dir.path()).await.unwrap();
        p.add_session(SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into())).await.unwrap();
        let p2 = StatePersistence::load(dir.path()).await.unwrap();
        assert_eq!(p2.session_count().await, 1);
    }

    #[tokio::test]
    async fn test_rollback_on_mutation_error() {
        let (_, p) = setup().await;
        p.add_session(SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into())).await.unwrap();
        let result = p.try_update_session("S01", |s| {
            s.turn = 99;
            TransitionValidator::validate(s, StreamStatus::Complete)
        }).await;
        assert!(result.is_err());
        assert_eq!(p.get_session("S01").await.unwrap().turn, 0);
    }

    #[tokio::test]
    async fn test_get_worktree_path() {
        let (_, p) = setup().await;
        p.add_session(SessionState::new("S01".into(), "deepseek".into(), "/custom/wt".into())).await.unwrap();
        assert_eq!(p.get_worktree_path("S01").await, Some("/custom/wt".into()));
        assert_eq!(p.get_worktree_path("nonexistent").await, None);
    }
}
```

## src/context/mod.rs

```rust
pub mod budget;
pub mod truncation;
pub mod assembler;
pub use budget::TokenBudget;
pub use assembler::ContextAssembler;
```

## src/context/budget.rs

```rust
/// Token budget tracker with 10% safety margin on estimates.
pub struct TokenBudget {
    pub max_tokens: usize,
    pub used_tokens: usize,
}

impl TokenBudget {
    pub fn new(max_tokens: usize) -> Self { Self { max_tokens, used_tokens: 0 } }
    pub fn remaining(&self) -> usize { self.max_tokens.saturating_sub(self.used_tokens) }
    pub fn try_allocate(&mut self, tokens: usize) -> bool {
        if self.used_tokens + tokens <= self.max_tokens { self.used_tokens += tokens; true } else { false }
    }
    pub fn force_allocate(&mut self, tokens: usize) { self.used_tokens += tokens; }

    /// Estimate tokens with a 10% safety margin.
    /// This is approximate, not exact. For exact counts, use the
    /// model's tokenizer.
    pub fn estimate_tokens(text: &str) -> usize {
        (text.len() * 11 + 27) / 40
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_safety_margin() {
        let estimate = TokenBudget::estimate_tokens(&"x".repeat(1000));
        let raw = (1000 + 3) / 4;
        assert!(estimate > raw, "safety margin should increase: {estimate} vs {raw}");
        assert!(estimate < raw * 2, "should not double: {estimate}");
    }

    #[test]
    fn test_budget_allocation() {
        let mut b = TokenBudget::new(100);
        assert!(b.try_allocate(50));
        assert!(!b.try_allocate(51));
    }
}
```

## src/context/truncation.rs

```rust
//! Smart truncation preserving structural lines.
//! Uses `Peekable<Chars>` instead of `Vec<char>` to avoid allocation.

pub fn smart_truncate(content: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= max_lines { return content.to_string(); }

    let mut result = Vec::new();
    let mut brace_depth: usize = 0;

    for line in &lines {
        let trimmed = line.trim();
        let is_fn_sig = trimmed.starts_with("fn ")
            || trimmed.starts_with("async fn ")
            || trimmed.starts_with("pub fn ")
            || trimmed.starts_with("pub async fn ")
            || trimmed.starts_with("pub(crate) fn ")
            || trimmed.starts_with("pub(crate) async fn ")
            || trimmed.starts_with("pub const fn ")
            || trimmed.starts_with("macro_rules! ");
        let is_structural = trimmed.starts_with("pub ")
            || trimmed.starts_with("struct ") || trimmed.starts_with("enum ")
            || trimmed.starts_with("trait ") || trimmed.starts_with("type ")
            || trimmed.starts_with("const ") || trimmed.starts_with("use ")
            || trimmed.starts_with("mod ") || trimmed.starts_with("#[")
            || trimmed.starts_with("///") || trimmed.starts_with("//!")
            || trimmed.is_empty();

        let (opens, closes) = count_braces(trimmed);

        if is_fn_sig {
            if brace_depth > 0 { result.push("    ...".to_string()); result.push("}".to_string()); brace_depth = 0; }
            result.push((*line).to_string());
            brace_depth = brace_depth.saturating_add(opens).saturating_sub(closes);
        } else if is_structural {
            result.push((*line).to_string());
            brace_depth = brace_depth.saturating_add(opens).saturating_sub(closes);
        } else if brace_depth > 0 {
            brace_depth = brace_depth.saturating_add(opens).saturating_sub(closes);
            if brace_depth == 0 { result.push("    ...".to_string()); result.push("}".to_string()); }
        } else {
            brace_depth = brace_depth.saturating_add(opens).saturating_sub(closes);
            if brace_depth == 0 { result.push((*line).to_string()); }
        }
        if result.len() >= max_lines { result.push("// ... (truncated)".to_string()); break; }
    }
    if brace_depth > 0 { result.push("    ...".to_string()); result.push("}".to_string()); }
    result.join("\n")
}

fn count_braces(s: &str) -> (usize, usize) {
    let mut opens = 0;
    let mut closes = 0;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') { break; } // line comment
        if c == '/' && chars.peek() == Some(&'*') { // block comment
            chars.next();
            let mut prev_star = false;
            while let Some(next) = chars.next() {
                if prev_star && next == '/' { break; }
                prev_star = next == '*';
            }
            continue;
        }
        if c == 'r' { // raw string
            let next = chars.peek().copied();
            if next == Some('"') || next == Some('#') {
                let mut hash_count = 0usize;
                while chars.peek() == Some(&'#') { chars.next(); hash_count += 1; }
                if chars.peek() == Some(&'"') {
                    chars.next();
                    let mut prev_quote = false;
                    let mut hash_matched = 0;
                    loop {
                        match chars.next() {
                            None => break,
                            Some('"') => { prev_quote = true; hash_matched = 0; }
                            Some('#') if prev_quote => { hash_matched += 1; if hash_matched == hash_count { break; } }
                            Some(_) => { prev_quote = false; hash_matched = 0; }
                        }
                    }
                    continue;
                }
                continue;
            }
        }
        if c == '"' { // regular string
            while let Some(next) = chars.next() {
                if next == '\\' { chars.next(); continue; }
                if next == '"' { break; }
            }
            continue;
        }
        if c == '\'' { // char literal
            while let Some(next) = chars.next() {
                if next == '\\' { chars.next(); continue; }
                if next == '\'' { break; }
            }
            continue;
        }
        if c == '{' { opens += 1; } else if c == '}' { closes += 1; }
    }
    (opens, closes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_braces_string_literal() { assert_eq!(count_braces(r#"let s = "a { b";"#), (0, 0)); }
    #[test]
    fn test_count_braces_raw_string() { assert_eq!(count_braces(r#"let s = r#"has { }"#;"#), (0, 0)); }
    #[test]
    fn test_count_braces_line_comment() { assert_eq!(count_braces("// { not counted"), (0, 0)); }
    #[test]
    fn test_smart_truncate_short() { assert_eq!(smart_truncate("fn main() {}", 100), "fn main() {}"); }
}
```

## src/context/assembler.rs

**Fix #4 from critique**: Budget overrun fixed — the spawn_blocking closure now tracks a running `used` counter and checks `tokens <= remaining` instead of `tokens <= max_budget` independently for each file.

```rust
use super::budget::TokenBudget;
use super::truncation::smart_truncate;
use crate::config::types::PilotConfig;
use crate::error::PilotError;
use std::path::Path;
use std::sync::Arc;

const DEFAULT_MAX_LINES: usize = 800;
const TEST_PREVIEW_LINES: usize = 30;
const ORCHESTRATION_START: &str = "<!-- orchestration-start -->";
const ORCHESTRATION_END: &str = "<!-- orchestration-end -->";

#[derive(Debug, Clone)]
pub struct AssembledContext {
    pub prompt: String,
    pub total_tokens: usize,
    pub tier1_tokens: usize,
    pub tier2_tokens: usize,
    pub tier3_tokens: usize,
}

pub trait FileReader: Send + Sync {
    fn read_to_string(&self, path: &Path) -> Option<String>;
}

pub struct DiskFileReader;
impl FileReader for DiskFileReader {
    fn read_to_string(&self, path: &Path) -> Option<String> { std::fs::read_to_string(path).ok() }
}

pub struct ContextAssembler {
    project_root: std::path::PathBuf,
    config: Arc<PilotConfig>,
    master_context: Option<String>,
    contracts_content: Option<String>,
    file_reader: Arc<dyn FileReader>,
}

impl ContextAssembler {
    pub async fn new(project_root: std::path::PathBuf, config: Arc<PilotConfig>) -> Self {
        let master_context = tokio::fs::read_to_string(project_root.join("AGENT_MASTER_CONTEXT.md")).await.ok();
        let contracts_content = tokio::fs::read_to_string(project_root.join("CONTRACTS_LOCKED.md")).await.ok();
        Self { project_root, config, master_context, contracts_content, file_reader: Arc::new(DiskFileReader) }
    }

    pub fn new_with_reader(
        project_root: std::path::PathBuf, config: Arc<PilotConfig>, file_reader: Arc<dyn FileReader>,
    ) -> Self {
        let master_context = file_reader.read_to_string(&project_root.join("AGENT_MASTER_CONTEXT.md"));
        let contracts_content = file_reader.read_to_string(&project_root.join("CONTRACTS_LOCKED.md"));
        Self { project_root, config, master_context, contracts_content, file_reader }
    }

    pub async fn assemble(
        &self, stream_id: &str, owned_files: &[String],
        dependency_interfaces: &[String], test_files: &[String], provider_id: &str,
    ) -> Result<AssembledContext, PilotError> {
        let max_tokens = self.config.context.providers.get(provider_id)
            .map(|c| c.max_context_tokens)
            .unwrap_or(self.config.context.max_context_tokens);
        let mut budget = TokenBudget::new(max_tokens);
        let mut prompt = String::new();

        // Tier 1: Master context + contracts
        let tier1 = self.assemble_tier1()?;
        let tier1_tokens = TokenBudget::estimate_tokens(&tier1);
        budget.force_allocate(tier1_tokens);
        prompt.push_str(&tier1);

        // Tier 2: Owned files — single spawn_blocking with RUNNING budget check
        let tier2_content = self.assemble_tier2(owned_files, test_files, budget.remaining()).await?;
        let tier2_tokens = TokenBudget::estimate_tokens(&tier2_content);
        budget.force_allocate(tier2_tokens);
        prompt.push_str(&tier2_content);

        // Tier 3: Dependency interfaces
        let mut tier3_content = String::new();
        for dep in dependency_interfaces {
            let section = format!("\n### Dependency: {dep}\n```rust\n// pub signatures only\n```\n");
            if budget.try_allocate(TokenBudget::estimate_tokens(&section)) { tier3_content.push_str(&section); }
        }
        let tier3_tokens = TokenBudget::estimate_tokens(&tier3_content);
        budget.force_allocate(tier3_tokens);
        prompt.push_str(&tier3_content);

        prompt.push_str("\n\n## Output Format\nRespond with ```glyim-ops``` blocks using ::WRITE, ::REPLACE, ::DELETE, ::COMMIT, ::INCOMPLETE, ::DONE, and ::APPROVED directives.\n");

        Ok(AssembledContext {
            prompt, total_tokens: budget.used_tokens,
            tier1_tokens, tier2_tokens, tier3_tokens,
        })
    }

    /// Fix #4: assemble_tier2 now receives the REMAINING budget (not the
    /// total max) and tracks a running `used` counter inside the closure.
    /// Each file is checked against `remaining - used`, not `max_budget`
    /// independently. This prevents two files each using 80% of budget
    /// from passing the check but exceeding the total by 60%.
    async fn assemble_tier2(
        &self, owned_files: &[String], test_files: &[String], remaining_budget: usize,
    ) -> Result<String, PilotError> {
        let file_reader = self.file_reader.clone();
        let project_root = self.project_root.clone();
        let owned = owned_files.to_vec();
        let tests = test_files.to_vec();

        let content = tokio::task::spawn_blocking(move || {
            let mut result = String::new();
            // FIX: Track running usage inside the closure
            let mut used: usize = 0;

            for file_path in &owned {
                if remaining_budget == 0 { break; }
                let full_path = project_root.join(file_path);
                if let Some(content) = file_reader.read_to_string(&full_path) {
                    let truncated = smart_truncate(&content, DEFAULT_MAX_LINES);
                    let section = format!("\n### {file_path}\n```rust\n{truncated}\n```\n");
                    let tokens = TokenBudget::estimate_tokens(&section);
                    // FIX: Check against remaining - used, not remaining alone
                    if used + tokens <= remaining_budget {
                        result.push_str(&section);
                        used += tokens;
                    } else {
                        tracing::warn!(path = %file_path, tokens, remaining = remaining_budget - used, "skipping owned file: exceeds remaining budget");
                    }
                } else {
                    tracing::warn!(path = %file_path, "failed to read owned file");
                }
            }

            for test_path in &tests {
                if remaining_budget == 0 { break; }
                let full_path = project_root.join(test_path);
                if let Some(content) = file_reader.read_to_string(&full_path) {
                    let preview: Vec<&str> = content.lines().take(TEST_PREVIEW_LINES).collect();
                    let section = format!("\n### {test_path} (preview)\n```rust\n{}\n// ...\n```\n", preview.join("\n"));
                    let tokens = TokenBudget::estimate_tokens(&section);
                    if used + tokens <= remaining_budget {
                        result.push_str(&section);
                        used += tokens;
                    }
                }
            }
            result
        }).await.map_err(|e| PilotError::Session(format!("spawn_blocking: {e}")))?;

        Ok(content)
    }

    fn assemble_tier1(&self) -> Result<String, PilotError> {
        let mut tier1 = String::from("# Glyim Compiler Development\n\n");
        if let Some(ref content) = self.master_context {
            tier1.push_str(&strip_orchestration(content));
            tier1.push('\n');
        }
        if let Some(ref content) = self.contracts_content {
            tier1.push_str("## Locked Contracts\n\n");
            tier1.push_str(content);
            tier1.push('\n');
        }
        tier1.push_str("\n## File Operations Skill\nUse ::WRITE <path> to create/replace files, ::REPLACE <path> with ---FIND--- / ---REPLACE--- to edit, ::DELETE <path> to remove.\nEnd each file content with ::END. Use ::COMMIT <msg> to request a commit, ::INCOMPLETE if still generating, ::DONE when finished.\n");
        Ok(tier1)
    }
}

fn strip_orchestration(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut in_orch = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == ORCHESTRATION_START { in_orch = true; continue; }
        if trimmed == ORCHESTRATION_END { in_orch = false; continue; }
        if !in_orch { result.push_str(line); result.push('\n'); }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_orchestration() {
        let content = "before\n<!-- orchestration-start -->\ngit worktree add ...\n<!-- orchestration-end -->\nafter\ncargo fmt is ok\n";
        let stripped = strip_orchestration(content);
        assert!(stripped.contains("before"));
        assert!(stripped.contains("after"));
        assert!(stripped.contains("cargo fmt is ok"));
        assert!(!stripped.contains("git worktree"));
    }
}
```

## src/dispatch/mod.rs

```rust
pub mod provider_pool;
pub mod rate_limit;
pub mod wave;

pub use provider_pool::ProviderPool;
pub use rate_limit::{handle_rate_limit, RateLimitAction, RateLimitContext};
pub use wave::{dispatch_wave, DispatchStrategy, StreamAssignment};
```

## src/dispatch/provider_pool.rs

```rust
use crate::config::types::ProviderConfig;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ProviderPool { providers: HashMap<String, ProviderState> }

#[derive(Debug, Clone)]
struct ProviderState {
    config: Arc<ProviderConfig>,
    active_slots: usize,
    cooldown_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct SlotAllocation { pub provider_id: String, pub available_slots: usize }

impl ProviderPool {
    pub fn new(providers: &HashMap<String, ProviderConfig>) -> Self {
        let mut states = HashMap::new();
        for (id, config) in providers {
            if config.enabled {
                states.insert(id.clone(), ProviderState { config: Arc::new(config.clone()), active_slots: 0, cooldown_until: None });
            }
        }
        Self { providers: states }
    }

    pub fn allocate(&mut self, provider_id: &str) -> Result<(), String> {
        let state = self.providers.get_mut(provider_id).ok_or_else(|| format!("provider {provider_id} not found"))?;
        if state.in_cooldown() { return Err(format!("provider {provider_id} in cooldown")); }
        if state.active_slots >= state.config.max_concurrent { return Err(format!("no available slots for {provider_id}")); }
        state.active_slots += 1;
        Ok(())
    }

    pub fn free(&mut self, provider_id: &str) {
        if let Some(state) = self.providers.get_mut(provider_id) { state.active_slots = state.active_slots.saturating_sub(1); }
    }

    pub fn cooldown(&mut self, provider_id: &str, duration_secs: u64) {
        if let Some(state) = self.providers.get_mut(provider_id) {
            state.cooldown_until = Some(Utc::now() + Duration::seconds(duration_secs.min(i64::MAX as u64) as i64));
        }
    }

    pub fn most_slots_available(&self) -> Option<SlotAllocation> {
        self.providers.iter()
            .filter(|(_, s)| !s.in_cooldown() && s.active_slots < s.config.max_concurrent)
            .max_by_key(|(_, s)| s.config.max_concurrent - s.active_slots)
            .map(|(id, s)| SlotAllocation { provider_id: id.clone(), available_slots: s.config.max_concurrent - s.active_slots })
    }

    pub fn available_slots(&self, provider_id: &str) -> usize {
        self.providers.get(provider_id).map(|s| s.config.max_concurrent.saturating_sub(s.active_slots)).unwrap_or(0)
    }

    pub fn is_in_cooldown(&self, provider_id: &str) -> bool {
        self.providers.get(provider_id).map(|s| s.in_cooldown()).unwrap_or(false)
    }

    pub fn provider_ids(&self) -> Vec<String> { self.providers.keys().cloned().collect() }
    pub fn get_config(&self, provider_id: &str) -> Option<Arc<ProviderConfig>> { self.providers.get(provider_id).map(|s| s.config.clone()) }
    pub fn total_available_slots(&self) -> usize {
        self.providers.values().filter(|s| !s.in_cooldown()).map(|s| s.config.max_concurrent.saturating_sub(s.active_slots)).sum()
    }
}

impl ProviderState {
    fn in_cooldown(&self) -> bool { self.cooldown_until.map_or(false, |until| Utc::now() < until) }
}
```

## src/dispatch/rate_limit.rs

```rust
use crate::dispatch::provider_pool::ProviderPool;
use crate::error::PilotError;

#[derive(Debug, Clone)]
pub enum RateLimitAction {
    Failover { new_provider_id: String, failover_prompt: String },
    RetryAfter { provider_id: String, delay_secs: u64 },
    Escalate { reason: String },
}

#[derive(Debug, Clone)]
pub struct RateLimitContext {
    pub stream_id: String, pub turn: u32, pub commits: u32,
    pub brief_summary: String, pub max_reassign_attempts: u32,
}

pub fn handle_rate_limit(
    pool: &mut ProviderPool, provider_id: &str, base_delay_secs: u64,
    max_delay_secs: u64, attempt: u32, ctx: &RateLimitContext,
) -> Result<RateLimitAction, PilotError> {
    let cooldown = pool.get_config(provider_id).map(|c| c.rate_limit_cooldown).unwrap_or(base_delay_secs);
    pool.cooldown(provider_id, cooldown);
    tracing::warn!(provider_id, cooldown_secs = cooldown, attempt, "rate limit detected");

    if attempt <= ctx.max_reassign_attempts {
        if let Some(allocation) = pool.most_slots_available() {
            if allocation.provider_id != provider_id {
                return Ok(RateLimitAction::Failover {
                    new_provider_id: allocation.provider_id,
                    failover_prompt: format!("Session {} moved from {} due to rate limit. Turns: {}, Commits: {}. Brief: {}",
                        ctx.stream_id, provider_id, ctx.turn, ctx.commits, ctx.brief_summary),
                });
            }
        }
    }

    let delay = calculate_staggered_backoff(base_delay_secs, max_delay_secs, attempt);
    if attempt < 5 { Ok(RateLimitAction::RetryAfter { provider_id: provider_id.to_string(), delay_secs: delay }) }
    else { Ok(RateLimitAction::Escalate { reason: format!("rate limit on {provider_id} after {attempt} attempts") }) }
}

fn calculate_staggered_backoff(base: u64, max: u64, attempt: u32) -> u64 {
    let exp = base.saturating_mul(2u64.saturating_pow(attempt)).min(max);
    let stagger = (attempt as u64 * 17) % ((exp as f64 * 0.2).max(1.0) as u64);
    exp.saturating_add(stagger).min(max)
}
```

## src/dispatch/wave.rs

```rust
use crate::dispatch::provider_pool::ProviderPool;
use crate::error::PilotError;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub enum DispatchStrategy { MostSlotsFirst, RoundRobin, LeastLoaded }

impl std::str::FromStr for DispatchStrategy {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "most_slots_first" => Ok(Self::MostSlotsFirst),
            "round_robin" => Ok(Self::RoundRobin),
            "least_loaded" => Ok(Self::LeastLoaded),
            _ => Err(format!("unknown strategy: {s}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamAssignment { pub stream_id: String, pub provider_id: String }

pub fn dispatch_wave(
    stream_ids: &[String], pool: &mut ProviderPool, strategy: &DispatchStrategy,
) -> Result<Vec<StreamAssignment>, PilotError> {
    let mut unassigned: VecDeque<String> = stream_ids.iter().cloned().collect();
    let mut assignments = Vec::new();

    match strategy {
        DispatchStrategy::MostSlotsFirst => {
            while let Some(best) = pool.most_slots_available() {
                if pool.allocate(&best.provider_id).is_ok() {
                    if let Some(id) = unassigned.pop_front() {
                        assignments.push(StreamAssignment { stream_id: id, provider_id: best.provider_id });
                    }
                } else { break; }
            }
        }
        DispatchStrategy::RoundRobin => {
            let providers = pool.provider_ids();
            if providers.is_empty() { return Ok(assignments); }
            let mut idx = 0;
            let mut fails = 0;
            while let Some(id) = unassigned.pop_front() {
                let pid = &providers[idx % providers.len()];
                if pool.allocate(pid).is_ok() { assignments.push(StreamAssignment { stream_id: id, provider_id: pid.clone() }); fails = 0; }
                else { unassigned.push_front(id); fails += 1; if fails > providers.len() * 2 { break; } }
                idx += 1;
            }
        }
        DispatchStrategy::LeastLoaded => {
            while let Some(id) = unassigned.pop_front() {
                let mut providers = pool.provider_ids();
                providers.sort_by(|a, b| pool.available_slots(b).cmp(&pool.available_slots(a)));
                let mut allocated = false;
                for pid in &providers {
                    if pool.allocate(pid).is_ok() { assignments.push(StreamAssignment { stream_id: id, provider_id: pid.clone() }); allocated = true; break; }
                }
                if !allocated { break; }
            }
        }
    }
    Ok(assignments)
}
```

## src/server/mod.rs

```rust
pub mod messages;
pub mod ws;
pub mod event_handler;

pub use messages::{ExtensionMessage, CliMessage};
pub use ws::{ServerEvent, WsServer};
```

## src/server/messages.rs

```rust
use crate::protocol::types::PROTOCOL_VERSION;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExtensionMessage {
    #[serde(rename = "session.ready", rename_all = "camelCase")]
    SessionReady { session_id: String, provider_id: String, tab_id: u64, #[serde(default)] trace_id: Option<String>, #[serde(default)] v: u32 },
    #[serde(rename = "ops.ready", rename_all = "camelCase")]
    OpsReady { session_id: String, content: String, turn: u32, #[serde(default)] trace_id: Option<String>, #[serde(default)] v: u32 },
    #[serde(rename = "stream.complete", rename_all = "camelCase")]
    StreamComplete { session_id: String, turn: u32, full_response: String, #[serde(default)] trace_id: Option<String>, #[serde(default)] v: u32 },
    #[serde(rename = "error.detected", rename_all = "camelCase")]
    ErrorDetected { session_id: String, error_type: String, error_message: String, recoverable: bool, #[serde(default)] trace_id: Option<String>, #[serde(default)] v: u32 },
    #[serde(rename = "pong")] Pong { timestamp: u64, #[serde(default)] v: u32 },
}

impl ExtensionMessage {
    pub fn version(&self) -> u32 {
        match self { Self::SessionReady { v, .. } | Self::OpsReady { v, .. } | Self::StreamComplete { v, .. } | Self::ErrorDetected { v, .. } | Self::Pong { v, .. } => *v }
    }
    pub fn session_id(&self) -> Option<&str> {
        match self { Self::SessionReady { session_id, .. } | Self::OpsReady { session_id, .. } | Self::StreamComplete { session_id, .. } | Self::ErrorDetected { session_id, .. } => Some(session_id), Self::Pong { .. } => None }
    }
    pub fn trace_id(&self) -> Option<&str> {
        match self { Self::SessionReady { trace_id, .. } | Self::OpsReady { trace_id, .. } | Self::StreamComplete { trace_id, .. } | Self::ErrorDetected { trace_id, .. } => trace_id.as_deref(), Self::Pong { .. } => None }
    }

    /// Reject v == 0 (missing version). Warn on mismatch.
    pub fn validate_version(&self) -> Result<(), String> {
        let v = self.version();
        if v == 0 { return Err(format!("message with v=0 rejected — protocol version required (current: {})", PROTOCOL_VERSION)); }
        if v > PROTOCOL_VERSION { tracing::warn!(msg_version = v, server_version = PROTOCOL_VERSION, "message from newer protocol version"); }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CliMessage {
    #[serde(rename = "session.start", rename_all = "camelCase")]
    SessionStart { session_id: String, provider_id: String, prompt: String, system_prompt: String, #[serde(default)] trace_id: Option<String>, #[serde(default)] v: u32 },
    #[serde(rename = "feedback.send", rename_all = "camelCase")]
    FeedbackSend { session_id: String, message: String, turn: u32, #[serde(default)] trace_id: Option<String>, #[serde(default)] v: u32 },
    #[serde(rename = "feedback.continue", rename_all = "camelCase")]
    FeedbackContinue { session_id: String, #[serde(default)] trace_id: Option<String>, #[serde(default)] v: u32 },
    #[serde(rename = "retry.prompt", rename_all = "camelCase")]
    RetryPrompt { session_id: String, message: String, delay: u64, #[serde(default)] trace_id: Option<String>, #[serde(default)] v: u32 },
    #[serde(rename = "session.pause", rename_all = "camelCase")]
    SessionPause { session_id: String, #[serde(default)] trace_id: Option<String>, #[serde(default)] v: u32 },
    #[serde(rename = "session.abort", rename_all = "camelCase")]
    SessionAbort { session_id: String, #[serde(default)] trace_id: Option<String>, #[serde(default)] v: u32 },
    #[serde(rename = "ping")] Ping { timestamp: u64, #[serde(default)] v: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reject_version_zero() {
        let msg = ExtensionMessage::Pong { timestamp: 123, v: 0 };
        assert!(msg.validate_version().is_err());
    }

    #[test]
    fn test_accept_valid_version() {
        let msg = ExtensionMessage::Pong { timestamp: 123, v: PROTOCOL_VERSION };
        assert!(msg.validate_version().is_ok());
    }

    #[test]
    fn test_camelcase_serialization() {
        let msg = CliMessage::FeedbackSend { session_id: "s1".into(), message: "err".into(), turn: 2, trace_id: None, v: 1 };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"type\":\"feedback.send\""));
    }
}
```

## src/server/ws.rs

```rust
use crate::error::PilotError;
use crate::server::messages::ExtensionMessage;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};

/// Bounded channel capacity (Fix #12): prevents OOM under load.
const EVENT_CHANNEL_CAPACITY: usize = 1024;

#[derive(Debug, Clone)]
pub enum ServerEvent {
    Connected { addr: SocketAddr },
    Message { session_id: Option<String>, trace_id: Option<String>, msg: ExtensionMessage },
    Disconnected { addr: SocketAddr },
}

pub struct WsServer {
    addr: SocketAddr,
    event_tx: mpsc::Sender<ServerEvent>,
    event_rx: Option<mpsc::Receiver<ServerEvent>>,
    cli_msg_tx: broadcast::Sender<String>,
}

impl WsServer {
    pub fn new(host: &str, port: u16) -> Self {
        let addr: SocketAddr = format!("{host}:{port}").parse().expect("invalid bind address");
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let (cli_msg_tx, _) = broadcast::channel(256);
        Self { addr, event_tx, event_rx: Some(event_rx), cli_msg_tx }
    }

    pub fn take_event_rx(&mut self) -> Option<mpsc::Receiver<ServerEvent>> { self.event_rx.take() }
    pub fn cli_msg_sender(&self) -> broadcast::Sender<String> { self.cli_msg_tx.clone() }

    pub async fn run(&self) -> Result<(), PilotError> {
        let listener = TcpListener::bind(&self.addr).await?;
        tracing::info!("WebSocket server listening on ws://{}", self.addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    if !addr.ip().is_loopback() {
                        tracing::error!(peer = %addr, "REJECTED non-localhost connection");
                        continue;
                    }
                    let event_tx = self.event_tx.clone();
                    let cli_msg_rx = self.cli_msg_tx.subscribe();
                    tokio::spawn(async move {
                        let ws_stream = match tokio_tungstenite::accept_async(stream).await {
                            Ok(ws) => ws, Err(e) => { tracing::warn!(peer = %addr, "handshake failed: {e}"); return; }
                        };
                        tracing::info!(peer = %addr, "extension connected");
                        let _ = event_tx.send(ServerEvent::Connected { addr }).await;
                        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

                        // Outgoing sender task
                        let send_tx = event_tx.clone();
                        let mut send_rx = cli_msg_rx;
                        let send_addr = addr;
                        tokio::spawn(async move {
                            while let Ok(msg) = send_rx.recv().await {
                                if ws_sender.send(tokio_tungstenite::tungstenite::Message::Text(msg.into())).await.is_err() { break; }
                            }
                            let _ = send_tx.send(ServerEvent::Disconnected { addr: send_addr }).await;
                        });

                        // Incoming receiver
                        let recv_tx = event_tx.clone();
                        let recv_addr = addr;
                        while let Some(msg) = ws_receiver.next().await {
                            match msg {
                                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                                    match serde_json::from_str::<ExtensionMessage>(&text) {
                                        Ok(ext_msg) => {
                                            if let Err(e) = ext_msg.validate_version() {
                                                tracing::warn!(peer = %recv_addr, "rejecting: {e}");
                                                continue;
                                            }
                                            let sid = ext_msg.session_id().map(|s| s.to_string());
                                            let tid = ext_msg.trace_id().map(|s| s.to_string());
                                            if let Err(e) = recv_tx.send(ServerEvent::Message { session_id: sid, trace_id: tid, msg: ext_msg }).await {
                                                tracing::warn!(peer = %recv_addr, "channel full: {e}");
                                            }
                                        }
                                        Err(e) => { tracing::debug!(peer = %recv_addr, "parse error: {e}"); }
                                    }
                                }
                                Ok(tokio_tungstenite::tungstenite::Message::Ping(data)) => {
                                    let _ = ws_sender.send(tokio_tungstenite::tungstenite::Message::Pong(data)).await;
                                }
                                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => break,
                                Ok(_) => { /* Pong, Binary, Frame — logged at debug elsewhere */ }
                                Err(e) => { tracing::debug!(peer = %recv_addr, "ws error: {e}"); break; }
                            }
                        }
                        tracing::info!(peer = %recv_addr, "extension disconnected");
                        let _ = recv_tx.send(ServerEvent::Disconnected { addr: recv_addr }).await;
                    });
                }
                Err(e) => { tracing::error!("accept failed: {e}"); tokio::time::sleep(tokio::time::Duration::from_millis(100)).await; }
            }
        }
    }
}
```

## src/server/event_handler.rs

**Fix #2 from critique**: The old module computed results and silently discarded them. Now `map_action_to_cli_message` is a pure function used by main.rs. The module does NOT spawn its own tasks — main.rs is the single point that spawns tasks and sends CLI messages.

```rust
//! Event handler utilities.
//!
//! This module provides the `map_action_to_cli_message` function used
//! by main.rs to convert OrchestratorActions into CLI messages.
//!
//! **Design decision**: The spawn + send happens in main.rs, NOT here.
//! The previous version spawned tasks inside this module but couldn't
//! access the cli_sender (it wasn't moved into the spawn), so all
//! messages were silently dropped. Now the module is stateless — it
//! only maps actions to messages. The caller is responsible for
//! spawning and sending.

use crate::orchestrator::OrchestratorAction;
use crate::protocol::types::PROTOCOL_VERSION;
use crate::server::messages::CliMessage;

/// Map an OrchestratorAction to an optional CliMessage.
/// Returns None for WaitForResponse (logged at debug by caller).
pub fn map_action_to_cli_message(action: OrchestratorAction, turn: u32) -> Option<CliMessage> {
    match action {
        OrchestratorAction::Feedback { session_id, message, trace_id } => Some(CliMessage::FeedbackSend {
            session_id, message, turn: turn + 1, trace_id, v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::Continue { session_id, trace_id } => Some(CliMessage::FeedbackContinue {
            session_id, trace_id, v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::SelfReview { session_id, prompt, trace_id } => Some(CliMessage::SessionStart {
            session_id, provider_id: "self_review".into(), prompt,
            system_prompt: "You are a code reviewer. Respond with ::APPROVED or fix issues.".into(),
            trace_id, v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::StreamComplete { session_id, pr_url, trace_id } => Some(CliMessage::FeedbackSend {
            session_id, message: format!("Stream complete! PR: {}", pr_url), turn: turn + 1, trace_id, v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::Escalate { session_id, reason, trace_id } => Some(CliMessage::FeedbackSend {
            session_id, message: format!("ESCALATION: {}", reason), turn: turn + 1, trace_id, v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::WaitForResponse { .. } => {
            // Caller logs this at debug level — no CLI message needed
            None
        }
    }
}
```

## src/orchestrator/mod.rs

```rust
pub mod turn;
pub use turn::{OrchestratorAction, TurnContext, process_turn_dispatch};
```

## src/orchestrator/turn.rs

**Fix #9 from critique**: `trace_id` is `String` (not `Option<String>`) in TurnContext since it's always populated before the struct is created.

```rust
//! Turn processing: the core orchestrator loop.

use crate::applier::apply_ops_async;
use crate::commit::{CommitContext, CommitDecision, CommitEngine};
use crate::config::types::PilotConfig;
use crate::error::PilotError;
use crate::gates::done_pipeline;
use crate::gates::self_review::build_review_prompt;
use crate::git_ops::{create_pr, diff_main, diff_name_only, log_oneline, push_branch};
use crate::metrics::Metrics;
use crate::protocol::parser::parse_ops_block;
use crate::session::machine::TransitionValidator;
use crate::session::persistence::StatePersistence;
use crate::session::state::StreamStatus;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub enum OrchestratorAction {
    Feedback { session_id: String, message: String, trace_id: Option<String> },
    Continue { session_id: String, trace_id: Option<String> },
    SelfReview { session_id: String, prompt: String, trace_id: Option<String> },
    StreamComplete { session_id: String, pr_url: String, trace_id: Option<String> },
    Escalate { session_id: String, reason: String, trace_id: Option<String> },
    WaitForResponse { session_id: String, trace_id: Option<String> },
}

/// TurnContext bundles all parameters for process_turn_dispatch.
///
/// **Fix #9**: `trace_id` is a `String`, not `Option<String>`. It is
/// always populated by the caller before the struct is created. This
/// eliminates `unwrap()` / `as_deref()` throughout the orchestrator.
pub struct TurnContext {
    pub ops_block: String,
    pub session_id: String,
    pub stream_id: String,
    pub worktree_dir: PathBuf,
    pub project_root: PathBuf,
    pub config: Arc<PilotConfig>,
    pub persistence: Arc<StatePersistence>,
    pub processing: Arc<Mutex<HashSet<String>>>,
    pub turn: u32,
    /// Always populated. The caller generates a UUID if the extension
    /// didn't provide one.
    pub trace_id: String,
    pub metrics: Arc<dyn Metrics>,
}

/// Process a turn. No catch_unwind — we rely on JoinError from
/// tokio::spawn, which actually catches async panics.
pub async fn process_turn_dispatch(ctx: TurnContext) -> Result<OrchestratorAction, PilotError> {
    let span = tracing::info_span!("process_turn", stream_id = %ctx.stream_id, turn = ctx.turn, trace_id = %ctx.trace_id);

    // Concurrency guard: acquire
    {
        let _enter = span.enter();
        let mut guard = ctx.processing.lock().await;
        if !guard.insert(ctx.stream_id.clone()) {
            tracing::warn!(stream_id = %ctx.stream_id, "already processing, skipping duplicate");
            return Ok(OrchestratorAction::WaitForResponse {
                session_id: ctx.session_id.clone(),
                trace_id: Some(ctx.trace_id.clone()),
            });
        }
    }

    let stream_id_clone = ctx.stream_id.clone();
    let processing_clone = ctx.processing.clone();
    let metrics_clone = ctx.metrics.clone();

    let result = tokio::spawn(async move { process_turn_inner(ctx).await }).await;

    // ALWAYS remove from processing set
    { let mut guard = processing_clone.lock().await; guard.remove(&stream_id_clone); }

    match result {
        Ok(inner_result) => { metrics_clone.increment_counter("turn_processed", &[]); inner_result }
        Err(join_error) => {
            let reason = if join_error.is_panic() { "task panicked".into() } else { "task cancelled".into() };
            tracing::error!(stream_id = %stream_id_clone, reason = %reason, "processing task failed");
            metrics_clone.increment_counter("turn_panic", &[]);
            Err(PilotError::Session(format!("processing failed for stream {}: {}", stream_id_clone, reason)))
        }
    }
}

async fn process_turn_inner(ctx: TurnContext) -> Result<OrchestratorAction, PilotError> {
    let ops = parse_ops_block(&ctx.ops_block)?;
    tracing::info!(ops_count = ops.ops.len(), "parsed ops block");

    // Drive session state
    ctx.persistence.try_update_session(&ctx.stream_id, |s| {
        match s.status {
            StreamStatus::Init => { TransitionValidator::transition(s, StreamStatus::Seeding)?; TransitionValidator::transition(s, StreamStatus::Waiting)?; }
            StreamStatus::Feedback | StreamStatus::Committed => { TransitionValidator::transition(s, StreamStatus::Waiting)?; }
            _ => {}
        }
        Ok(())
    }).await?;

    // Apply file operations
    if !ops.ops.is_empty() {
        ctx.persistence.try_update_session(&ctx.stream_id, |s| {
            TransitionValidator::transition(s, StreamStatus::Executing)?;
            Ok(())
        }).await?;
        let results = apply_ops_async(ctx.worktree_dir.clone(), ops.ops.clone(), ctx.config.limits.clone()).await?;
        tracing::info!(applied = results.len(), "file operations applied");
        ctx.metrics.record_histogram("ops_applied", results.len() as f64, &[]);
    }

    let trace_id_some = Some(ctx.trace_id.clone());

    if ops.approved { return handle_approved(&ctx, &trace_id_some).await; }
    if ops.done { return handle_done(&ctx, &trace_id_some).await; }
    if ops.incomplete {
        ctx.persistence.try_update_session(&ctx.stream_id, |s| { s.record_turn(); Ok(()) }).await?;
        return Ok(OrchestratorAction::Continue { session_id: ctx.session_id.clone(), trace_id: trace_id_some });
    }
    if let Some(msg) = ops.commit_message { return handle_commit(&ctx, &msg, &trace_id_some).await; }

    ctx.persistence.try_update_session(&ctx.stream_id, |s| { s.record_turn(); Ok(()) }).await?;
    Ok(OrchestratorAction::WaitForResponse { session_id: ctx.session_id.clone(), trace_id: trace_id_some })
}

async fn handle_commit(
    ctx: &TurnContext, commit_message: &str, trace_id: &Option<String>,
) -> Result<OrchestratorAction, PilotError> {
    let current_fix_round = ctx.persistence.get_fix_round(&ctx.stream_id).await;

    ctx.persistence.try_update_session(&ctx.stream_id, |s| {
        TransitionValidator::transition(s, StreamStatus::Committing)?;
        Ok(())
    }).await?;

    let resolved = ctx.config.gates.commit.resolve(
        ctx.config.gates.level, ctx.config.execution.default_branch.clone(), ctx.config.execution.branch_version.clone(),
    );

    // Compute changed files ONCE and pass to the commit engine
    let changed_files: Vec<String> = diff_name_only(
        &ctx.worktree_dir, &ctx.config.execution.default_branch, ctx.config.execution.command_timeout,
    ).await.unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();

    let engine = CommitEngine::new(resolved, ctx.config.execution.max_fix_rounds, ctx.config.gates.banned_patterns.clone(), ctx.config.gates.architecture_rules.clone());

    let commit_ctx = CommitContext {
        worktree_dir: ctx.worktree_dir.clone(), project_root: ctx.project_root.clone(),
        stream_id: ctx.stream_id.clone(), commit_message: commit_message.to_string(),
        current_fix_round, timeout_secs: ctx.config.execution.command_timeout,
        default_branch: ctx.config.execution.default_branch.clone(),
        branch_version: ctx.config.execution.branch_version.clone(),
        changed_files,
    };

    let decision = engine.evaluate_commit(&commit_ctx).await?;

    if matches!(decision, CommitDecision::Escalated { .. }) {
        if let Err(e) = engine.emergency_commit(&commit_ctx).await {
            tracing::error!(error = %e, "emergency commit failed");
        }
    }

    ctx.persistence.try_update_session(&ctx.stream_id, |s| {
        if s.fix_round != current_fix_round {
            return Err(PilotError::Session(format!("fix_round changed: {} vs {}", current_fix_round, s.fix_round)));
        }
        match &decision {
            CommitDecision::Committed { new_fix_round, .. } => { s.record_commit(); s.fix_round = *new_fix_round; TransitionValidator::transition(s, StreamStatus::Committed)?; }
            CommitDecision::GateFailed { new_fix_round, .. } => { s.fix_round = *new_fix_round; TransitionValidator::transition(s, StreamStatus::Feedback)?; }
            CommitDecision::Escalated { new_fix_round, .. } => { s.fix_round = *new_fix_round; TransitionValidator::transition(s, StreamStatus::Error)?; }
        }
        Ok(())
    }).await?;

    ctx.metrics.increment_counter("commit_decision", &[("result", match &decision {
        CommitDecision::Committed { .. } => "committed",
        CommitDecision::GateFailed { .. } => "gate_failed",
        CommitDecision::Escalated { .. } => "escalated",
    })]);

    match decision {
        CommitDecision::Committed { message, .. } => Ok(OrchestratorAction::Feedback {
            session_id: ctx.session_id.clone(), message: format!("✅ Committed: {}", message), trace_id: trace_id.clone(),
        }),
        CommitDecision::GateFailed { feedback, .. } => Ok(OrchestratorAction::Feedback {
            session_id: ctx.session_id.clone(), message: format!("❌ Commit gate failed:\n\n{}", feedback), trace_id: trace_id.clone(),
        }),
        CommitDecision::Escalated { feedback, .. } => Ok(OrchestratorAction::Escalate {
            session_id: ctx.session_id.clone(), reason: format!("Fix rounds exceeded.\n\n{}", feedback), trace_id: trace_id.clone(),
        }),
    }
}

async fn handle_done(ctx: &TurnContext, trace_id: &Option<String>) -> Result<OrchestratorAction, PilotError> {
    let resolved = ctx.config.gates.done.resolve(ctx.config.gates.level);
    let changed_files: Vec<String> = diff_name_only(
        &ctx.worktree_dir, &ctx.config.execution.default_branch, ctx.config.execution.command_timeout,
    ).await.unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();

    let gate_ctx = crate::gates::types::GateContext::new(
        ctx.worktree_dir.clone(), ctx.project_root.clone(),
        ctx.config.execution.default_branch.clone(), ctx.config.execution.branch_version.clone(),
        ctx.config.execution.command_timeout, changed_files,
    );
    let result = done_pipeline::run_done_pipeline(&gate_ctx, &resolved).await?;

    if result.passed {
        let diff = diff_main(&ctx.worktree_dir, &ctx.config.execution.default_branch, ctx.config.execution.command_timeout).await?;
        let log = log_oneline(&ctx.worktree_dir, &ctx.config.execution.default_branch, ctx.config.execution.command_timeout).await?;
        ctx.persistence.try_update_session(&ctx.stream_id, |s| {
            TransitionValidator::transition(s, StreamStatus::Verifying)?;
            TransitionValidator::transition(s, StreamStatus::Reviewing)?;
            Ok(())
        }).await?;
        Ok(OrchestratorAction::SelfReview { session_id: ctx.session_id.clone(), prompt: build_review_prompt(&diff, &log), trace_id: trace_id.clone() })
    } else {
        ctx.persistence.try_update_session(&ctx.stream_id, |s| { TransitionValidator::transition(s, StreamStatus::Feedback)?; Ok(()) }).await?;
        Ok(OrchestratorAction::Feedback { session_id: ctx.session_id.clone(), message: format!("❌ Done gate failed:\n\n{}", result.failure_message()), trace_id: trace_id.clone() })
    }
}

async fn handle_approved(ctx: &TurnContext, trace_id: &Option<String>) -> Result<OrchestratorAction, PilotError> {
    push_branch(&ctx.worktree_dir, &ctx.stream_id, &ctx.config.execution.branch_version, ctx.config.execution.command_timeout).await?;
    let title = format!("stream-{}: implementation", ctx.stream_id);
    let body = format!("Automated implementation for stream {}", ctx.stream_id);
    let pr_url = create_pr(&ctx.worktree_dir, &ctx.stream_id, &ctx.config.execution.default_branch, &ctx.config.execution.branch_version, &title, &body, ctx.config.execution.command_timeout).await?;
    ctx.persistence.try_update_session(&ctx.stream_id, |s| { TransitionValidator::transition(s, StreamStatus::Complete)?; Ok(()) }).await?;
    ctx.metrics.increment_counter("pr_created", &[]);
    Ok(OrchestratorAction::StreamComplete { session_id: ctx.session_id.clone(), pr_url, trace_id: trace_id.clone() })
}
```

## src/cli/mod.rs

```rust
pub mod dashboard;
pub mod preflight;
pub use dashboard::{render_status_table, render_wave_summary};
```

## src/cli/dashboard.rs

```rust
use crate::session::state::{SessionState, StreamStatus};
use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, Table};

pub fn render_status_table(sessions: &[SessionState]) -> String {
    if sessions.is_empty() { return "No active sessions.".to_string(); }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Stream", "Provider", "Status", "Turn", "Fixes", "Commits", "Last Activity"]);
    for s in sessions {
        let color = match s.status { StreamStatus::Complete => Color::Green, StreamStatus::Error => Color::Red, StreamStatus::Paused => Color::Yellow, StreamStatus::Streaming | StreamStatus::Executing => Color::Cyan, _ => Color::White };
        table.add_row(vec![
            Cell::new(&s.stream_id), Cell::new(&s.provider_id),
            Cell::new(format!("{:?}", s.status)).fg(color),
            Cell::new(s.turn), Cell::new(s.fix_round), Cell::new(s.commits),
            Cell::new(s.last_activity.format("%H:%M:%S")),
        ]);
    }
    table.to_string()
}

pub fn render_wave_summary(sessions: &[SessionState]) -> String {
    if sessions.is_empty() { return "No sessions in wave.".to_string(); }
    let total_turns: u32 = sessions.iter().map(|s| s.turn).sum();
    let total_commits: u32 = sessions.iter().map(|s| s.commits).sum();
    let completed = sessions.iter().filter(|s| s.status == StreamStatus::Complete).count();
    format!("Summary: {completed}/{} complete, {} total turns, {} total commits", sessions.len(), total_turns, total_commits)
}
```

## src/cli/preflight.rs

```rust
use crate::config::PilotConfig;
use std::sync::Arc;

pub async fn run_preflight(config: &Arc<PilotConfig>) {
    println!("Running preflight checks...");
    match tokio::process::Command::new("git").args(["--version"]).output().await {
        Ok(o) if o.status.success() => println!("✅ git: {}", String::from_utf8_lossy(&o.stdout).trim()),
        _ => println!("❌ git: not found"),
    }
    match tokio::process::Command::new("cargo").args(["--version"]).output().await {
        Ok(o) if o.status.success() => println!("✅ cargo: {}", String::from_utf8_lossy(&o.stdout).trim()),
        _ => println!("❌ cargo: not found"),
    }
    println!("Providers: {} configured", config.providers.len());
    println!("Gate level: {}", config.gates.level);
    println!("Default branch: {} ({})", config.execution.default_branch, config.execution.branch_version);
}
```

## src/main.rs

**Fix #2 from critique**: No duplicate event handler. main.rs calls `server::event_handler::map_action_to_cli_message` and properly clones + moves the cli_sender into spawned tasks. Messages are never silently dropped.

```rust
use clap::{Parser, Subcommand};
use glyim_pilot::cli::{render_status_table, run_preflight};
use glyim_pilot::config::{self, PilotConfig};
use glyim_pilot::metrics::production_metrics;
use glyim_pilot::protocol::types::PROTOCOL_VERSION;
use glyim_pilot::server::{CliMessage, ExtensionMessage, ServerEvent, WsServer};
use glyim_pilot::session::persistence::StatePersistence;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(name = "glyim-pilot", version = "0.3.0")]
struct Cli {
    #[arg(long, env = "GLYIM_PROJECT_ROOT", default_value = ".")]
    project_root: PathBuf,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands { Serve, Status, Preflight }

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = match config::load_config(&cli.project_root) {
        Ok(c) => Arc::new(c),
        Err(e) => { eprintln!("Config error: {e}"); std::process::exit(1); }
    };

    match cli.command {
        Commands::Serve => run_serve(config, cli.project_root).await,
        Commands::Status => run_status(cli.project_root).await,
        Commands::Preflight => run_preflight(&config).await,
    }
}

async fn run_serve(config: Arc<PilotConfig>, project_root: PathBuf) {
    let mut server = WsServer::new(&config.server.host, config.server.port);
    let mut event_rx = server.take_event_rx().expect("event rx already taken");
    let cli_sender = server.cli_msg_sender();
    let server = Arc::new(server);
    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        if let Err(e) = server_clone.run().await { tracing::error!("Server error: {e}"); }
    });

    let persistence = Arc::new(StatePersistence::load(&project_root).await.expect("failed to load state"));
    let processing: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let metrics: Arc<dyn glyim_pilot::metrics::Metrics> = production_metrics();

    tracing::info!("Glym Pilot server started on ws://{}:{}", config.server.host, config.server.port);

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => { tracing::info!("Shutting down..."); break; }
            Some(event) = event_rx.recv() => {
                match event {
                    ServerEvent::Connected { addr } => tracing::info!(peer = %addr, "extension connected"),
                    ServerEvent::Disconnected { addr } => tracing::info!(peer = %addr, "extension disconnected"),
                    ServerEvent::Message { msg, .. } => {
                        handle_extension_message(
                            msg, &config, &persistence, &project_root,
                            &cli_sender, &processing, &metrics,
                        ).await;
                    }
                }
            }
        }
    }
}

/// Single handler for extension messages. No duplicate logic.
/// The cli_sender is cloned and moved into spawned tasks so
/// messages are NEVER silently dropped.
async fn handle_extension_message(
    msg: ExtensionMessage,
    config: &Arc<PilotConfig>,
    persistence: &Arc<StatePersistence>,
    project_root: &PathBuf,
    cli_sender: &tokio::sync::broadcast::Sender<String>,
    processing: &Arc<Mutex<HashSet<String>>>,
    metrics: &Arc<dyn glyim_pilot::metrics::Metrics>,
) {
    match msg {
        ExtensionMessage::SessionReady { session_id, provider_id, tab_id, .. } => {
            tracing::info!(session_id, provider_id, tab_id, "session ready");
        }
        ExtensionMessage::OpsReady { session_id, content, turn, trace_id, .. } => {
            // Always generate trace ID (Fix #15)
            let trace_id = trace_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            let worktree_path = persistence.get_worktree_path(&session_id).await;
            let worktree_dir = match worktree_path {
                Some(path) => PathBuf::from(path),
                None => {
                    tracing::error!(session_id, "worktree_path not found");
                    let err_msg = CliMessage::FeedbackSend {
                        session_id: session_id.clone(), message: "Internal error: worktree path not found".into(),
                        turn: turn + 1, trace_id: Some(trace_id), v: PROTOCOL_VERSION,
                    };
                    let _ = cli_sender.send(serde_json::to_string(&err_msg).unwrap());
                    return;
                }
            };

            let stream_id = persistence.get_stream_id(&session_id).await.unwrap_or_else(|| session_id.clone());

            let turn_ctx = glyim_pilot::orchestrator::TurnContext {
                ops_block: content,
                session_id,
                stream_id,
                worktree_dir,
                project_root: project_root.clone(),
                config: Arc::clone(config),
                persistence: Arc::clone(persistence),
                processing: Arc::clone(processing),
                turn,
                trace_id, // String, not Option<String> (Fix #9)
                metrics: Arc::clone(metrics),
            };

            // FIX: Clone cli_sender and move it into the spawned task.
            // The previous version had the spawn inside event_handler.rs
            // but couldn't access the sender — all messages were silently
            // dropped. Now the spawn happens HERE with a proper clone.
            let cli_sender_clone = cli_sender.clone();
            let metrics_clone = Arc::clone(metrics);

            tokio::spawn(async move {
                metrics_clone.increment_counter("ops_ready_received", &[]);

                match glyim_pilot::orchestrator::process_turn_dispatch(turn_ctx).await {
                    Ok(action) => {
                        if let Some(cli_msg) = glyim_pilot::server::event_handler::map_action_to_cli_message(action, turn) {
                            let json = serde_json::to_string(&cli_msg).unwrap();
                            if let Err(e) = cli_sender_clone.send(json) {
                                tracing::warn!("failed to send CLI message: {e}");
                            }
                        } else {
                            tracing::debug!("orchestrator waiting for response — no CLI message needed");
                        }
                    }
                    Err(e) => {
                        tracing::error!(?e, "orchestrator error");
                        metrics_clone.increment_counter("orchestrator_error", &[("code", e.code())]);
                    }
                }
            });
        }
        ExtensionMessage::StreamComplete { session_id, turn, .. } => {
            tracing::info!(session_id, turn, "stream complete");
            metrics.increment_counter("stream_complete", &[]);
        }
        ExtensionMessage::ErrorDetected { session_id, error_type, error_message, recoverable, trace_id, .. } => {
            tracing::warn!(session_id, error_type, error_message, recoverable, "error from extension");
            metrics.increment_counter("extension_error", &[("type", &error_type)]);
            if recoverable {
                let response = CliMessage::FeedbackSend {
                    session_id: session_id.clone(), message: format!("Provider error: {}", error_message),
                    turn: 0, trace_id, v: PROTOCOL_VERSION,
                };
                let _ = cli_sender.send(serde_json::to_string(&response).unwrap());
            }
        }
        ExtensionMessage::Pong { timestamp, .. } => { tracing::debug!(timestamp, "pong"); }
    }
}

async fn run_status(project_root: PathBuf) {
    let persistence = StatePersistence::load(&project_root).await.expect("failed to load state");
    let sessions = persistence.all_sessions().await;
    if sessions.is_empty() { println!("No sessions found."); }
    else { println!("{}", render_status_table(&sessions)); }
}
```

## Chrome Extension

### extension/src/providers/adapter.ts

**Fix #11 from critique**: Added `customSetInput` escape hatch for providers that need non-standard input handling (like Gemini's contenteditable). **Fix #5**: `ConfigurableAdapter.setInput` now handles `contenteditable="true"` elements using `document.execCommand` fallback.

```typescript
/** Fix #18: Data-driven provider configuration.
 *  Fix #11: customSetInput escape hatch for non-standard providers.
 *  Fix #5: contenteditable handling restored for Gemini. */

export interface ProviderError {
  type: 'rate_limit' | 'server_busy' | 'capacity' | 'server_error' | 'network_error';
  message: string;
  recoverable: boolean;
}

export interface ProviderAdapter {
  readonly id: string;
  readonly urlPattern: RegExp;
  readonly assistantSelector: string;
  readonly homepageUrl: string;
  setInput(text: string): Promise<void>;
  submitMessage(): Promise<void>;
  isStreaming(): boolean;
  getCodeBlocks(): string[];
  detectError(): ProviderError | null;
  getAssistantText(): string;
}

/** Configuration that parameterizes a provider adapter.
 *  Most providers differ only in CSS selectors, URLs, and streaming
 *  indicators. customSetInput provides an escape hatch for providers
 *  that genuinely differ (e.g. Gemini's contenteditable input). */
export interface ProviderConfig {
  id: string;
  urlPattern: RegExp;
  homepageUrl: string;
  inputSelector: string;
  assistantSelector: string;
  streamingSelector: string;
  errorSelectors: string[];
  /** Optional override for setInput. When present, this function
   *  replaces the default behavior entirely. Use this for providers
   *  with non-standard input elements (e.g. contenteditable divs). */
  customSetInput?: (text: string) => Promise<void>;
}

const adapterRegistry: ProviderAdapter[] = [];
export function registerAdapter(adapter: ProviderAdapter): void { adapterRegistry.push(adapter); }
export function getAdapterForUrl(url: string): ProviderAdapter | null {
  return adapterRegistry.find(a => a.urlPattern.test(url)) ?? null;
}
export function getAllAdapters(): ProviderAdapter[] { return [...adapterRegistry]; }

export function insertText(element: HTMLTextAreaElement | HTMLInputElement, text: string): void {
  const start = element.selectionStart ?? 0;
  const end = element.selectionEnd ?? 0;
  element.setRangeText(text, start, end, 'end');
  element.dispatchEvent(new Event('input', { bubbles: true }));
}

export async function clickSendWhenEnabled(maxWaitMs = 5000): Promise<void> {
  const pollInterval = 100;
  const maxAttempts = maxWaitMs / pollInterval;
  for (let i = 0; i < maxAttempts; i++) {
    const btn = document.querySelector<HTMLButtonElement>(
      "button[type='submit'], button[aria-label*='send'], div[class*='send-button']"
    );
    if (btn && !btn.disabled && !btn.getAttribute('aria-disabled')) { btn.click(); return; }
    await new Promise(r => setTimeout(r, pollInterval));
  }
  throw new Error('send button not found or not enabled within timeout');
}

/** Set input text into an element, handling both textarea/input and
 *  contenteditable elements. Fix #5: contenteditable uses
 *  document.execCommand('insertText') as a fallback. */
export async function setInputText(selector: string, text: string): Promise<void> {
  const element = document.querySelector<HTMLElement>(selector);
  if (!element) throw new Error(`input not found by selector: ${selector}`);
  element.focus();

  if (element instanceof HTMLTextAreaElement || element instanceof HTMLInputElement) {
    insertText(element, text);
  } else if (element.isContentEditable) {
    // Fix #5: Gemini and similar providers use contenteditable divs.
    // insertText via execCommand works for these elements.
    document.execCommand('insertText', false, text);
  } else {
    // Last resort: try execCommand anyway
    document.execCommand('insertText', false, text);
  }
}

export class ConfigurableAdapter implements ProviderAdapter {
  readonly id: string;
  readonly urlPattern: RegExp;
  readonly assistantSelector: string;
  readonly homepageUrl: string;
  private readonly config: ProviderConfig;

  constructor(config: ProviderConfig) {
    this.config = config;
    this.id = config.id;
    this.urlPattern = config.urlPattern;
    this.assistantSelector = config.assistantSelector;
    this.homepageUrl = config.homepageUrl;
  }

  async setInput(text: string): Promise<void> {
    // Fix #11: Use customSetInput if provided, otherwise default
    if (this.config.customSetInput) {
      await this.config.customSetInput(text);
      return;
    }
    await setInputText(this.config.inputSelector, text);
  }

  async submitMessage(): Promise<void> { await clickSendWhenEnabled(); }
  isStreaming(): boolean { return document.querySelector(this.config.streamingSelector) !== null; }
  getCodeBlocks(): string[] { return Array.from(document.querySelectorAll('pre code')).map(b => b.textContent ?? ''); }

  detectError(): ProviderError | null {
    for (const selector of this.config.errorSelectors) {
      for (const el of document.querySelectorAll(selector)) {
        if (el.closest(this.assistantSelector)) continue;
        const text = el.textContent?.toLowerCase() ?? '';
        if (text.includes('rate limit') || text.includes('too frequent'))
          return { type: 'rate_limit', message: el.textContent?.trim() ?? '', recoverable: true };
        if (text.includes('capacity'))
          return { type: 'capacity', message: el.textContent?.trim() ?? '', recoverable: true };
        if (text.includes('server error'))
          return { type: 'server_error', message: el.textContent?.trim() ?? '', recoverable: true };
        if (text.includes('rate') || text.includes('limit'))
          return { type: 'rate_limit', message: el.textContent?.trim() ?? '', recoverable: true };
      }
    }
    return null;
  }

  getAssistantText(): string {
    const sel = this.config.assistantSelector;
    const lastEl = document.querySelector(`${sel}:last-of-type`);
    return lastEl?.textContent ?? '';
  }
}
```

### extension/src/providers/index.ts

```typescript
import { ConfigurableAdapter, registerAdapter, setInputText } from './adapter';

// DeepSeek
registerAdapter(new ConfigurableAdapter({
  id: 'deepseek',
  urlPattern: /chat\.deepseek\.com/,
  homepageUrl: 'https://chat.deepseek.com',
  inputSelector: "textarea[id='chat-input']",
  assistantSelector: '.ds-markdown--block',
  streamingSelector: '.typing-indicator',
  errorSelectors: ['.error-banner', '.toast-error', '[class*="error-message"]'],
}));

// z.ai
registerAdapter(new ConfigurableAdapter({
  id: 'zai',
  urlPattern: /z\.ai/,
  homepageUrl: 'https://z.ai',
  inputSelector: 'textarea',
  assistantSelector: '.message-assistant',
  streamingSelector: '.streaming, .loading',
  errorSelectors: ['[role="alert"]', '.error-message'],
}));

// Gemini — Fix #5: contenteditable handling.
// The inputSelector matches both textarea AND contenteditable.
// setInputText() in adapter.ts handles both element types.
// No customSetInput needed because the default now handles contenteditable.
registerAdapter(new ConfigurableAdapter({
  id: 'gemini',
  urlPattern: /gemini\.google\.com/,
  homepageUrl: 'https://gemini.google.com',
  inputSelector: 'textarea, [contenteditable="true"]',
  assistantSelector: 'model-response',
  streamingSelector: 'mat-progress-bar, .loading',
  errorSelectors: ['[role="alert"]', '.error-message'],
}));

// Grok — uses customSetInput as an example of the escape hatch
registerAdapter(new ConfigurableAdapter({
  id: 'grok',
  urlPattern: /grok\.x\.ai/,
  homepageUrl: 'https://grok.x.ai',
  inputSelector: 'textarea',
  assistantSelector: '.message-bubble.assistant',
  streamingSelector: '.typing-indicator, .streaming',
  errorSelectors: ['[role="alert"]', '.error-message'],
  // Example of customSetInput escape hatch (Fix #11):
  // customSetInput: async (text: string) => { /* custom logic */ }
}));

// Mistral
registerAdapter(new ConfigurableAdapter({
  id: 'mistral',
  urlPattern: /chat\.mistral\.ai/,
  homepageUrl: 'https://chat.mistral.ai',
  inputSelector: 'textarea',
  assistantSelector: '.prose',
  streamingSelector: '.loading, .streaming',
  errorSelectors: ['[role="alert"]', '.error-message'],
}));
```

### extension/src/types.ts

```typescript
export const PROTOCOL_VERSION = 1;

export interface SessionReady { type: 'session.ready'; sessionId: string; providerId: string; tabId: number; traceId?: string; v: number; }
export interface OpsReady { type: 'ops.ready'; sessionId: string; content: string; turn: number; traceId?: string; v: number; }
export interface StreamComplete { type: 'stream.complete'; sessionId: string; turn: number; fullResponse: string; traceId?: string; v: number; }
export interface ErrorDetected { type: 'error.detected'; sessionId: string; errorType: string; errorMessage: string; recoverable: boolean; traceId?: string; v: number; }
export interface Pong { type: 'pong'; timestamp: number; v: number; }
export type ExtensionMessage = SessionReady | OpsReady | StreamComplete | ErrorDetected | Pong;

export interface SessionStart { type: 'session.start'; sessionId: string; providerId: string; prompt: string; systemPrompt: string; traceId?: string; v: number; }
export interface FeedbackSend { type: 'feedback.send'; sessionId: string; message: string; turn: number; traceId?: string; v: number; }
export interface FeedbackContinue { type: 'feedback.continue'; sessionId: string; traceId?: string; v: number; }
export interface RetryPrompt { type: 'retry.prompt'; sessionId: string; message: string; delay: number; traceId?: string; v: number; }
export interface SessionPause { type: 'session.pause'; sessionId: string; traceId?: string; v: number; }
export interface SessionAbort { type: 'session.abort'; sessionId: string; traceId?: string; v: number; }
export interface Ping { type: 'ping'; timestamp: number; v: number; }
export type CliMessage = SessionStart | FeedbackSend | FeedbackContinue | RetryPrompt | SessionPause | SessionAbort | Ping;

export interface TabSession { tabId: number; sessionId: string; streamId: string; providerId: string; status: 'active' | 'paused' | 'error'; turn: number; }

export const DANGEROUS_PATTERNS: readonly string[] = ['rm -rf', 'git push', 'git reset --hard', 'cargo publish', 'sudo', 'chmod 777', 'mkfs', 'dd if='];

export function containsDangerousPattern(content: string): string | null {
  const lower = content.toLowerCase();
  for (const pattern of DANGEROUS_PATTERNS) { if (lower.includes(pattern.toLowerCase())) return pattern; }
  return null;
}

export function normalizeLineEndings(text: string): string { return text.replace(/\r/g, ''); }

export function validateMessageVersion(v: number | undefined): string | null {
  if (v === undefined || v === 0) return `message with v=${v ?? 'undefined'} rejected — protocol version required (current: ${PROTOCOL_VERSION})`;
  if (v > PROTOCOL_VERSION) return `message version ${v} > server version ${PROTOCOL_VERSION} — may not work`;
  return null;
}

export function serializeTabSessions(sessions: Map<number, TabSession>): string {
  const obj: Record<string, TabSession> = {};
  for (const [tabId, session] of sessions.entries()) { obj[String(tabId)] = session; }
  return JSON.stringify(obj);
}

export function deserializeTabSessions(raw: unknown): Map<number, TabSession> {
  const result = new Map<number, TabSession>();
  if (typeof raw !== 'object' || raw === null) return result;
  const obj = raw as Record<string, unknown>;
  for (const [key, value] of Object.entries(obj)) {
    const tabId = Number(key);
    if (!Number.isFinite(tabId)) continue;
    if (typeof value === 'object' && value !== null) result.set(tabId, value as TabSession);
  }
  return result;
}
```

### extension/src/ws_client.ts

```typescript
import type { ExtensionMessage, CliMessage } from './types';
import { PROTOCOL_VERSION, validateMessageVersion } from './types';

const DEFAULT_URL = 'ws://127.0.0.1:8420';
const RECONNECT_BASE_DELAY = 1000;
const RECONNECT_MAX_DELAY = 10000;
const PING_INTERVAL = 30000;

export class WsClient {
  private ws: WebSocket | null = null;
  private url: string;
  private reconnectAttempts = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private pingTimer: ReturnType<typeof setInterval> | null = null;
  private intentionalClose = false;
  private messageHandler: ((msg: CliMessage) => void) | null = null;
  private statusHandler: ((connected: boolean) => void) | null = null;

  constructor(url: string = DEFAULT_URL) { this.url = url; }
  onMessage(handler: (msg: CliMessage) => void): void { this.messageHandler = handler; }
  onStatusChange(handler: (connected: boolean) => void): void { this.statusHandler = handler; }
  connect(): void { this.intentionalClose = false; this.doConnect(); }
  disconnect(): void { this.intentionalClose = true; this.cleanup(); this.ws?.close(); this.ws = null; }
  send(msg: ExtensionMessage): boolean { if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return false; this.ws.send(JSON.stringify(msg)); return true; }
  get connected(): boolean { return this.ws !== null && this.ws.readyState === WebSocket.OPEN; }

  private doConnect(): void {
    try { this.ws = new WebSocket(this.url); } catch (e) { console.warn('glyim-pilot: WS creation failed:', e); this.scheduleReconnect(); return; }
    this.ws.onopen = () => { this.reconnectAttempts = 0; this.statusHandler?.(true); this.startPing(); };
    this.ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data) as CliMessage;
        const versionError = validateMessageVersion((msg as Record<string, unknown>).v as number | undefined);
        if (versionError) console.warn(`glyim-pilot: ${versionError}`);
        this.messageHandler?.(msg);
      } catch (e) { console.warn('glyim-pilot: failed to parse WS message:', e); }
    };
    this.ws.onclose = () => { this.statusHandler?.(false); this.stopPing(); this.ws = null; if (!this.intentionalClose) this.scheduleReconnect(); };
    this.ws.onerror = (e) => { console.warn('glyim-pilot: WS error:', e); };
  }

  private scheduleReconnect(): void {
    if (this.intentionalClose) return;
    const delay = Math.min(RECONNECT_BASE_DELAY * Math.pow(2, this.reconnectAttempts), RECONNECT_MAX_DELAY);
    this.reconnectAttempts++;
    this.reconnectTimer = setTimeout(() => this.doConnect(), delay);
  }
  private startPing(): void { this.stopPing(); this.pingTimer = setInterval(() => this.send({ type: 'ping', timestamp: Date.now(), v: PROTOCOL_VERSION }), PING_INTERVAL); }
  private stopPing(): void { if (this.pingTimer) clearInterval(this.pingTimer); }
  private cleanup(): void { if (this.reconnectTimer) clearTimeout(this.reconnectTimer); this.stopPing(); }
}
```

### extension/src/code_extractor.ts

```typescript
import { normalizeLineEndings } from './types';

export function extractGlyimOpsBlocks(response: string): string[] {
  const normalized = normalizeLineEndings(response);
  const blocks: string[] = [];
  const lines = normalized.split('\n');
  let i = 0;
  while (i < lines.length) {
    const trimmed = lines[i].trim();
    if (trimmed === '```glyim-ops' || trimmed.startsWith('```glyim-ops ')) {
      const contentStart = i + 1;
      let endLine = -1;
      let insideWriteOrReplace = false;
      for (let j = i + 1; j < lines.length; j++) {
        const t = lines[j].trim();
        if (t.startsWith('::WRITE ') || t.startsWith('::REPLACE ')) insideWriteOrReplace = true;
        else if (t === '::END' && insideWriteOrReplace) insideWriteOrReplace = false;
        if (t.startsWith('```') && !insideWriteOrReplace) { endLine = j; break; }
      }
      if (endLine >= 0) { blocks.push(lines.slice(contentStart, endLine).join('\n').trim()); i = endLine + 1; }
      else break;
    } else i++;
  }
  return blocks;
}

export function isBlockComplete(blockContent: string): boolean {
  const n = normalizeLineEndings(blockContent);
  return n.includes('::COMMIT') || n.includes('::DONE') || n.includes('::APPROVED') || n.includes('::INCOMPLETE');
}
```

### extension/src/stream_watcher.ts

```typescript
import type { ProviderAdapter } from './providers/adapter';
import { extractGlyimOpsBlocks, isBlockComplete } from './code_extractor';
import { containsDangerousPattern, normalizeLineEndings } from './types';

export class StreamWatcher {
  private observer: MutationObserver | null = null;
  private turn = 0;
  private previousResponseText = '';
  private sentHashes = new Set<string>();
  private isWatching = false;
  private pollingTimer: ReturnType<typeof setInterval> | null = null;
  private lastStreaming = false;
  private pendingCheck: Promise<void> = Promise.resolve();

  constructor(
    private adapter: ProviderAdapter,
    private sessionId: string,
    private onOpsReady: (content: string, turn: number) => void,
    private onStreamComplete: (fullResponse: string, turn: number) => void,
    private onDangerousPattern: (content: string, pattern: string) => void,
  ) {}

  start(): void {
    if (this.isWatching) return;
    this.isWatching = true;
    const container = document.querySelector('[role="main"]') ?? document.querySelector(this.adapter.assistantSelector)?.parentElement ?? document.body;
    this.observer = new MutationObserver(() => { if (!this.adapter.isStreaming()) void this.serializedCheck(); });
    this.observer.observe(container, { childList: true, subtree: true, characterData: true });
    this.pollingTimer = setInterval(() => {
      if (!this.isWatching) return;
      const streaming = this.adapter.isStreaming();
      if (this.lastStreaming && !streaming) { void this.serializedCheck(); this.handleStreamComplete(); }
      this.lastStreaming = streaming;
    }, 500);
  }

  stop(): void { this.isWatching = false; this.observer?.disconnect(); this.observer = null; if (this.pollingTimer) clearInterval(this.pollingTimer); this.pollingTimer = null; }
  resetForNewTurn(): void { this.turn++; this.previousResponseText = ''; }

  private async serializedCheck(): Promise<void> { this.pendingCheck = this.pendingCheck.then(() => this.checkForCompleteBlocks()); await this.pendingCheck; }

  private async checkForCompleteBlocks(): Promise<void> {
    try {
      const text = this.adapter.getAssistantText();
      if (!text || text === this.previousResponseText) return;
      this.previousResponseText = text;
      const blocks = extractGlyimOpsBlocks(normalizeLineEndings(text));
      for (const block of blocks) {
        const hash = await this.hash(block);
        if (this.sentHashes.has(hash)) continue;
        if (!isBlockComplete(block)) continue;
        const dangerous = containsDangerousPattern(block);
        if (dangerous) { this.onDangerousPattern(block, dangerous); this.sentHashes.add(hash); continue; }
        this.sentHashes.add(hash);
        this.onOpsReady(block, this.turn);
      }
    } catch (e) { console.warn('glyim-pilot: stream watcher check failed:', e); }
  }

  private handleStreamComplete(): void { const full = this.adapter.getAssistantText(); if (full) this.onStreamComplete(full, this.turn); this.sentHashes.clear(); }

  private async hash(content: string): Promise<string> {
    const data = new TextEncoder().encode(content);
    const hashBuffer = await crypto.subtle.digest('SHA-256', data);
    return Array.from(new Uint8Array(hashBuffer)).map(b => b.toString(16).padStart(2, '0')).join('').slice(0, 16);
  }
}
```

### extension/src/content.ts

```typescript
chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (msg.type === 'content.checkStatus') {
    sendResponse({ streaming: !!document.querySelector('.typing-indicator, .streaming, .loading, mat-progress-bar'), offline: !navigator.onLine });
  }
  if (msg.type === 'content.injectPrompt') {
    const prompt = msg.prompt as string;
    const input = document.querySelector<HTMLElement>('textarea, [contenteditable="true"]');
    if (!input) { sendResponse({ success: false, error: 'input not found' }); return true; }
    input.focus();
    if (input instanceof HTMLTextAreaElement || input instanceof HTMLInputElement) {
      const start = input.selectionStart ?? 0;
      const end = input.selectionEnd ?? 0;
      input.setRangeText(prompt, start, end, 'end');
      input.dispatchEvent(new Event('input', { bubbles: true }));
    } else if (input.isContentEditable) {
      // Fix #5: contenteditable handling
      document.execCommand('insertText', false, prompt);
    }
    sendResponse({ success: true });
  }
  return true;
});
```

### extension/src/background.ts

```typescript
import './providers/index';
import { WsClient } from './ws_client';
import { getAllAdapters } from './providers/adapter';
import { StreamWatcher } from './stream_watcher';
import type { CliMessage, TabSession } from './types';
import { PROTOCOL_VERSION, validateMessageVersion, serializeTabSessions, deserializeTabSessions } from './types';

const ws = new WsClient();
const tabSessions = new Map<number, TabSession>();
const watchers = new Map<number, StreamWatcher>();

ws.onMessage(async (msg: CliMessage) => {
  const versionError = validateMessageVersion((msg as Record<string, unknown>).v as number | undefined);
  if (versionError) console.warn(`glyim-pilot: ${versionError}`);
  try {
    switch (msg.type) {
      case 'session.start': await handleSessionStart(msg); break;
      case 'feedback.send': await handleFeedbackSend(msg); break;
      case 'feedback.continue': await handleFeedbackContinue(msg); break;
      case 'retry.prompt': await handleRetryPrompt(msg); break;
      case 'session.pause': await handleSessionPause(msg); break;
      case 'session.abort': await handleSessionAbort(msg); break;
      case 'ping': ws.send({ type: 'pong', timestamp: Date.now(), v: PROTOCOL_VERSION }); break;
    }
  } catch (e) { console.warn(`glym-pilot: error handling ${msg.type}:`, e); }
});

ws.onStatusChange(async (connected) => { if (connected) await restoreSessions(); });
ws.connect();

async function waitForInputElement(tabId: number, maxWaitMs = 10000): Promise<boolean> {
  for (let i = 0; i < maxWaitMs / 200; i++) {
    try {
      const results = await chrome.scripting.executeScript({ target: { tabId }, func: () => !!document.querySelector('textarea, [contenteditable="true"]') });
      if (results[0]?.result) return true;
    } catch { /* tab not ready */ }
    await new Promise(r => setTimeout(r, 200));
  }
  return false;
}

async function injectPrompt(tabId: number, prompt: string): Promise<{ success: boolean; error?: string }> {
  try {
    const results = await chrome.scripting.executeScript({
      target: { tabId },
      func: (text: string) => {
        const input = document.querySelector<HTMLElement>('textarea, [contenteditable="true"]');
        if (!input) return { success: false, error: 'input element not found' };
        input.focus();
        if (input instanceof HTMLTextAreaElement || input instanceof HTMLInputElement) {
          const start = input.selectionStart ?? 0; const end = input.selectionEnd ?? 0;
          input.setRangeText(text, start, end, 'end');
          input.dispatchEvent(new Event('input', { bubbles: true }));
        } else if (input.isContentEditable) {
          document.execCommand('insertText', false, text);
        }
        // Click send when enabled
        const pollForSend = (): void => {
          const btn = document.querySelector<HTMLButtonElement>("button[type='submit'], button[aria-label*='send']");
          if (btn && !btn.disabled) { btn.click(); return; }
          setTimeout(pollForSend, 100);
        };
        setTimeout(pollForSend, 50);
        return { success: true };
      },
      args: [prompt],
    });
    return results[0]?.result as { success: boolean; error?: string } ?? { success: false, error: 'no result' };
  } catch (e) { return { success: false, error: String(e) }; }
}

async function handleSessionStart(msg: Extract<CliMessage, { type: 'session.start' }>) {
  const { sessionId, providerId, prompt, traceId } = msg;
  const adapter = getAllAdapters().find(a => a.id === providerId);
  if (!adapter) { console.warn(`glym-pilot: no adapter for ${providerId}`); return; }
  const tab = await chrome.tabs.create({ url: adapter.homepageUrl, active: true });
  if (!tab.id) return;
  const ready = await waitForInputElement(tab.id);
  if (!ready) { ws.send({ type: 'error.detected', sessionId, errorType: 'input_not_found', errorMessage: 'Input element not found', recoverable: false, v: PROTOCOL_VERSION }); return; }
  const result = await injectPrompt(tab.id, prompt);
  if (!result.success) { ws.send({ type: 'error.detected', sessionId, errorType: 'injection_failed', errorMessage: result.error ?? 'unknown', recoverable: true, v: PROTOCOL_VERSION }); return; }
  tabSessions.set(tab.id, { tabId: tab.id, sessionId, streamId: sessionId, providerId, status: 'active', turn: 0 });
  await persistSessions();
  ws.send({ type: 'session.ready', sessionId, providerId, tabId: tab.id, traceId, v: PROTOCOL_VERSION });
  startWatcher(tab.id, sessionId, adapter);
}

async function handleFeedbackSend(msg: Extract<CliMessage, { type: 'feedback.send' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  await injectPrompt(entry[0], msg.message); watchers.get(entry[0])?.resetForNewTurn();
}

async function handleFeedbackContinue(msg: Extract<CliMessage, { type: 'feedback.continue' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  await injectPrompt(entry[0], 'Please continue.'); watchers.get(entry[0])?.resetForNewTurn();
}

async function handleRetryPrompt(msg: Extract<CliMessage, { type: 'retry.prompt' }>) {
  await new Promise(r => setTimeout(r, msg.delay));
  const entry = findSession(msg.sessionId); if (!entry) return;
  await injectPrompt(entry[0], msg.message);
}

async function handleSessionPause(msg: Extract<CliMessage, { type: 'session.pause' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  entry[1].status = 'paused'; watchers.get(entry[0])?.stop(); await persistSessions();
}

async function handleSessionAbort(msg: Extract<CliMessage, { type: 'session.abort' }>) {
  const entry = findSession(msg.sessionId); if (!entry) return;
  watchers.get(entry[0])?.stop(); watchers.delete(entry[0]); tabSessions.delete(entry[0]); await persistSessions();
}

function startWatcher(tabId: number, sessionId: string, adapter: ReturnType<typeof getAllAdapters>[0]) {
  watchers.get(tabId)?.stop();
  const watcher = new StreamWatcher(adapter, sessionId,
    (content, turn) => ws.send({ type: 'ops.ready', sessionId, content, turn, v: PROTOCOL_VERSION }),
    (full, turn) => ws.send({ type: 'stream.complete', sessionId, turn, fullResponse: full, v: PROTOCOL_VERSION }),
    (content, pattern) => ws.send({ type: 'error.detected', sessionId, errorType: 'dangerous_pattern', errorMessage: `Dangerous: "${pattern}"`, recoverable: true, v: PROTOCOL_VERSION }),
  );
  watcher.start(); watchers.set(tabId, watcher);
}

function findSession(sessionId: string): [number, TabSession] | null {
  for (const [tabId, sess] of tabSessions.entries()) { if (sess.sessionId === sessionId) return [tabId, sess]; }
  return null;
}

async function persistSessions() { await chrome.storage.local.set({ tabSessions: serializeTabSessions(tabSessions) }); }

async function restoreSessions() {
  const stored = await chrome.storage.local.get('tabSessions');
  if (!stored.tabSessions) return;
  try {
    const sessions = deserializeTabSessions(JSON.parse(stored.tabSessions as string));
    for (const [tabId, sess] of sessions.entries()) {
      try { await chrome.tabs.get(tabId); tabSessions.set(tabId, sess); const adapter = getAllAdapters().find(a => a.id === sess.providerId); if (adapter) startWatcher(tabId, sess.sessionId, adapter); }
      catch { /* tab gone */ }
    }
  } catch (e) { console.warn('glyim-pilot: failed to restore sessions:', e); }
}

chrome.runtime.onStartup.addListener(restoreSessions);
```

---

## Summary of All Critique Fixes

| # | Finding | Status | Where |
|---|---------|--------|-------|
| 1 | `src/process.rs` test: `err.kind` without binding `err` | ✅ Fixed | `let err = result.unwrap_err();` |
| 2 | `event_handler.rs` silently drops messages; duplicate in main.rs | ✅ Fixed | Module is now a pure `map_action_to_cli_message` function; main.rs spawns tasks and properly clones/moves `cli_sender` |
| 3 | PrometheusMetrics creates new counter on every increment | ✅ Fixed | `HashMap<String, IntCounter>` cache behind `Mutex`; first call creates+registers+ caches; subsequent calls increment cached counter |
| 4 | Budget overrun in `assemble_tier2` — each file checked independently against `max_budget` | ✅ Fixed | Running `used` counter inside closure; each file checks `used + tokens <= remaining_budget` |
| 5 | Gemini contenteditable input handling lost | ✅ Fixed | `setInputText()` handles `contenteditable` via `document.execCommand('insertText')`; content.ts also handles it |
| 6 | Fake debouncing — `DebouncedPersistence` claims debounce but flushes immediately | ✅ Fixed | Renamed to `StatePersistence`; deleted dead `start_flush_task`; honest immediate saves |
| 7 | Diff-based scanning contract not documented | ✅ Fixed | Doc comments on `BannedPatternGate` and `ArchitectureGate` explicitly state "checks changed files only" |
| 8 | `get_changed_files` called separately per gate | ✅ Fixed | `GateContext` now has `changed_files: Vec<String>` computed once by the pipeline |
| 9 | `trace_id` is `Option<String>` in TurnContext but always `Some` | ✅ Fixed | `TurnContext::trace_id` is now `String` |
| 10 | Side effects lost in `PipelineResult::failure_message()` | ✅ Fixed | `failure_message()` now appends side effects from passing gates |
| 11 | No escape hatch for per-provider input overrides | ✅ Fixed | `ProviderConfig.customSetInput?: (text: string) => Promise<void>` |
| 12 | `strip_string_literals` doesn't handle raw strings | ✅ Fixed | Full rewrite handles `r"..."`, `r#"..."#`, `b"..."`, `br"..."`, byte raw strings, line comments |
# Harsh Code Plan Critique: glyim-pilot v3

## Overall Sentiment

**Pass with Minor Fixes** — You systematically addressed every one of the 12 critique findings from v2, and the architecture is now genuinely solid: single event handler, honest persistence, cached Prometheus metrics, correct token budget tracking, restored contenteditable support, and pre-computed changed files. The code will work correctly in production. But you introduced two compilation errors and lost a tracing span, which means this plan cannot ship as-is. These are trivial to fix but must be fixed.

---

## Detailed Findings

### 1. Correctness & Compilation Safety — **6/10**

- **`src/config/types.rs` — `test_extract_deps` will fail at runtime**. The test uses a raw string `r#"[package]\nname = ..."#` but `\n` inside a raw string literal is the two characters `\` and `n`, not a newline. `toml::from_str` will fail to parse this malformed input, `extract_dependencies_toml` returns an empty `Vec`, and `assert!(deps.contains(&"glyim-ir".to_string()))` fails. **Fix**: Use a raw string with actual newlines (multi-line), or a regular string with `\n`.

- **`src/cli/mod.rs` — Missing re-export of `run_preflight`**. The module declares `pub mod preflight` but does not `pub use preflight::run_preflight;`. Meanwhile `src/main.rs` imports `use glyim_pilot::cli::{render_status_table, run_preflight};`. This is a **compilation error**. **Fix**: Add `pub use preflight::run_preflight;` to `cli/mod.rs`.

- **Tracing span not propagated into spawned task** (`src/orchestrator/turn.rs`). The span is created and entered only for the concurrency guard check, then the task is spawned without it:
  ```rust
  let span = tracing::info_span!("process_turn", stream_id = ..., turn = ..., trace_id = ...);
  { let _enter = span.enter(); /* guard check */ }
  let result = tokio::spawn(async move { process_turn_inner(ctx).await }).await;
  ```
  All logs inside `process_turn_inner` (and every function it calls) run **without** the span context — no `stream_id`, `turn`, or `trace_id` in any log line from the actual work. This is a meaningful observability regression. **Fix**: Use `.instrument(span)`:
  ```rust
  let result = tokio::spawn(process_turn_inner(ctx).instrument(span)).await;
  ```

- **Missing git_ops integration tests** — the v2 plan had `setup_test_repo` with real git operations. The v3 plan dropped them. These were valuable for catching real git behavior changes. Not a compilation issue but a test coverage regression.

### 2. Boundaries & Contracts — **8/10**

- **`GateContext` with pre-computed `changed_files`** is a clean solution. The contract is explicit: gates receive the changed file list; they don't make their own git calls. The doc comments on `banned_pattern.rs` and `architecture.rs` clearly state "checks changed files only." Good.

- **`StatePersistence` with no `lock()` exposure** — all access through specific methods. Clean.

- **`TurnContext.trace_id` as `String`** — eliminates the `Option` noise. Good.

- **`CommitContext.changed_files`** — properly threaded from the orchestrator through the commit engine to the gate context. No redundant git calls.

- **`customSetInput` escape hatch** — clean way to handle Gemini without special-casing in the adapter core.

- Minor: `GateContext::new` still takes 6 parameters. A builder would be cleaner but this is acceptable.

### 3. Modularity & Separation of Concerns — **8/10**

- **No duplicate event handler** — `event_handler.rs` is a pure `map_action_to_cli_message` function. `main.rs` is the single point that spawns tasks and sends messages. Clean.

- **`ConfigurableAdapter` with `customSetInput`** — eliminates 80% of provider boilerplate while preserving escape hatches.

- **`setInputText` as a shared utility** — handles both textarea and contenteditable. No more per-provider duplication.

- **`src/process.rs` as shared command runner** — both `git_ops` and `gates` wrap it. No duplication.

- Minor: The gate files (`check.rs`, `clippy.rs`, etc.) are shown inline in a combined code block rather than as separate files. This is a presentation issue, not an architectural one.

### 4. Performance & Resource Efficiency — **8/10**

- **PrometheusMetrics caching** — first call creates/registers/caches; subsequent calls hit the HashMap and increment. No per-call allocation. Correct.

- **Token budget tracking** — running `used` counter inside `spawn_blocking` prevents budget overrun. Correct.

- **Changed files computed once** — `GateContext.changed_files` is populated by the pipeline and shared across all gates. No redundant git calls.

- **`strip_string_literals` with `Peekable<Chars>`** — avoids `Vec<char>` allocation. Correct.

- **`count_braces` with `Peekable<Chars>`** — same optimization. Correct.

- Minor: `StatePersistence` uses `Mutex` (exclusive lock) even for reads. A `RwLock` would be better for read-heavy workloads, but the current single-event-loop usage makes this acceptable.

### 5. Debuggability & Observability — **6/10**

- **Tracing span regression** (see Correctness #3) — the most important observability feature (request-scoped trace context) is lost inside the spawned task. This means production debugging of orchestrator failures will be significantly harder. **This must be fixed.**

- **Side effects in `PipelineResult::failure_message()`** — the AI now knows about auto-fixes even when a later gate fails. Good.

- **Trace ID always present** — generated at entry point if not provided. Good.

- **Honest persistence naming** — `StatePersistence` doesn't pretend to debounce. Good.

- **Error codes with documentation** — `ERROR_CODES.md` is present and tested. Good.

- **All silent catches eliminated** — TypeScript `catch` blocks log warnings, Rust `_ => {}` branches log at debug level. Good.

### 6. Elegance & Hack-free Design — **8/10**

- **No fake debouncing** — renamed to `StatePersistence`, dead `start_flush_task` deleted. Honest.

- **No silently dropped messages** — `cli_sender` is properly cloned and moved into spawned tasks.

- **Proper contenteditable handling** — `setInputText` handles both element types.

- **Clean `map_action_to_cli_message`** — pure function, no hidden state.

- **`strip_string_literals` now handles raw/byte strings** — proper heuristic, not a hack.

- **No more `catch_unwind` on async** — relies on `JoinError`, which actually works.

- Minor: The `config/types.rs` one-liner formatting is dense but functionally correct.

---

## Summary of Required Fixes

1. **Fix `test_extract_deps` in `src/gates/architecture.rs`** — use actual newlines instead of `\n` inside a raw string.
2. **Add `pub use preflight::run_preflight;` to `src/cli/mod.rs`** — compilation error.
3. **Propagate the tracing span into the spawned task** in `src/orchestrator/turn.rs` — use `.instrument(span)` on the future before spawning.
4. **Restore git_ops integration tests** — at minimum `test_create_worktree` and `test_commit_all`.
5. **Add missing extension test files** — `code_extractor.test.ts` and `types.test.ts` from v2.
6. **Fix typo** in `extension/src/background.ts`: `glym-pilot` → `glyim-pilot`.

These are all small, localized fixes. The architecture is solid — ship after fixing.
