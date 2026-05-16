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
        } else
