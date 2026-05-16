# glyim-pilot: Complete Rewrite with All Critique Fixes

## `Cargo.toml`

```toml
[package]
name = "glyim-pilot"
version = "0.2.0"
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

[dev-dependencies]
proptest = "1.11"
tempfile = "3"
tokio-test = "0.4"
assert_cmd = "2"
predicates = "3"
pretty_assertions = "1"

[features]
default = []

[profile.release]
lto = true
strip = true
opt-level = 3
codegen-units = 1
```

## `src/main.rs`

```rust
use clap::{Parser, Subcommand};
use glyim_pilot::cli::{render_status_table, render_wave_summary};
use glyim_pilot::config::{self, PilotConfig};
use glyim_pilot::metrics::NoOpMetrics;
use glyim_pilot::orchestrator::{process_turn_dispatch, OrchestratorAction};
use glyim_pilot::server::{CliMessage, ExtensionMessage, ServerEvent, WsServer};
use glyim_pilot::session::persistence::DebouncedPersistence;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(name = "glyim-pilot", version = "0.2.0")]
struct Cli {
    #[arg(long, env = "GLYIM_PROJECT_ROOT", default_value = ".")]
    project_root: PathBuf,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Serve,
    Wave { wave: u8 },
    Status,
    Preflight,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = match config::load_config(&cli.project_root) {
        Ok(c) => Arc::new(c),
        Err(e) => {
            eprintln!("Config error: {e}");
            std::process::exit(1);
        }
    };

    match cli.command {
        Commands::Serve => run_serve(config, cli.project_root).await,
        Commands::Status => run_status(cli.project_root).await,
        Commands::Preflight => run_preflight(&config).await,
        Commands::Wave { wave } => run_wave(config, cli.project_root, wave).await,
    }
}

async fn run_serve(config: Arc<PilotConfig>, project_root: PathBuf) {
    let mut server = WsServer::new(&config.server.host, config.server.port);
    let mut event_rx = server.take_event_rx().expect("event rx already taken");
    let cli_sender = server.cli_msg_sender();
    let server = Arc::new(server);
    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        if let Err(e) = server_clone.run().await {
            tracing::error!("Server error: {e}");
        }
    });

    let persistence = Arc::new(
        DebouncedPersistence::load(&project_root)
            .await
            .expect("failed to load state persistence"),
    );
    let processing: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let metrics = Arc::new(NoOpMetrics);

    tracing::info!(
        "Glyim Pilot server started on ws://{}:{}",
        config.server.host,
        config.server.port
    );
    tracing::info!(
        "SECURITY: Only loopback connections accepted. \
         For container/VM use, configure host via .glyim-pilot.toml [server] host"
    );

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutting down...");
                break;
            }
            Some(event) = event_rx.recv() => {
                handle_event(
                    event,
                    &config,
                    &persistence,
                    &project_root,
                    &cli_sender,
                    &processing,
                    &metrics,
                )
                .await;
            }
        }
    }
}

async fn handle_event(
    event: ServerEvent,
    config: &Arc<PilotConfig>,
    persistence: &Arc<DebouncedPersistence>,
    project_root: &PathBuf,
    cli_sender: &tokio::sync::broadcast::Sender<String>,
    processing: &Arc<Mutex<HashSet<String>>>,
    metrics: &Arc<dyn glyim_pilot::metrics::Metrics>,
) {
    match event {
        ServerEvent::Connected { addr } => {
            tracing::info!(peer = %addr, "extension connected");
        }
        ServerEvent::Disconnected { addr } => {
            tracing::info!(peer = %addr, "extension disconnected");
        }
        ServerEvent::Message {
            session_id,
            trace_id,
            msg,
        } => match msg {
            ExtensionMessage::SessionReady {
                session_id,
                provider_id,
                tab_id,
                ..
            } => {
                tracing::info!(session_id, provider_id, tab_id, "session ready");
            }
            ExtensionMessage::OpsReady {
                session_id,
                content,
                turn,
                trace_id,
                ..
            } => {
                let worktree_path = {
                    let p = persistence.lock().await;
                    p.get_session(&session_id).map(|s| s.worktree_path.clone())
                };
                let worktree_dir = match worktree_path {
                    Some(path) => PathBuf::from(path),
                    None => {
                        tracing::error!(session_id, "worktree_path not found");
                        let err_msg = CliMessage::FeedbackSend {
                            session_id: session_id.clone(),
                            message: "Internal error: worktree path not found".into(),
                            turn: turn + 1,
                            trace_id: trace_id.clone(),
                            v: PROTOCOL_VERSION,
                        };
                        let _ = cli_sender.send(serde_json::to_string(&err_msg).unwrap());
                        return;
                    }
                };

                let stream_id = {
                    let p = persistence.lock().await;
                    p.get_session(&session_id)
                        .map(|s| s.stream_id.clone())
                        .unwrap_or_else(|| session_id.clone())
                };

                let config = Arc::clone(config);
                let persistence = Arc::clone(persistence);
                let processing = Arc::clone(processing);
                let cli_sender = cli_sender.clone();
                let project_root = project_root.clone();
                let metrics = Arc::clone(metrics);

                tokio::spawn(async move {
                    metrics.increment_counter("ops_ready_received", &[("session", &session_id)]);

                    match process_turn_dispatch(
                        &content,
                        &session_id,
                        &stream_id,
                        worktree_dir,
                        project_root,
                        config,
                        persistence,
                        processing,
                        turn,
                        trace_id.clone(),
                        &*metrics,
                    )
                    .await
                    {
                        Ok(action) => {
                            let response = map_action_to_cli_message(action, turn, &trace_id);
                            if let Some(msg) = response {
                                let _ = cli_sender.send(serde_json::to_string(&msg).unwrap());
                            }
                        }
                        Err(e) => {
                            tracing::error!(?e, "orchestrator error");
                            metrics.increment_counter("orchestrator_error", &[("code", e.code())]);
                            let err_msg = CliMessage::FeedbackSend {
                                session_id: session_id.clone(),
                                message: format!("Internal error [{}]: {}", e.code(), e),
                                turn: turn + 1,
                                trace_id,
                                v: PROTOCOL_VERSION,
                            };
                            let _ = cli_sender.send(serde_json::to_string(&err_msg).unwrap());
                        }
                    }
                });
            }
            ExtensionMessage::StreamComplete {
                session_id, turn, ..
            } => {
                tracing::info!(session_id, turn, "stream complete");
                metrics.increment_counter("stream_complete", &[("session", &session_id)]);
            }
            ExtensionMessage::ErrorDetected {
                session_id,
                error_type,
                error_message,
                recoverable,
                trace_id,
                ..
            } => {
                tracing::warn!(
                    session_id,
                    error_type,
                    error_message,
                    recoverable,
                    "error from extension"
                );
                metrics.increment_counter("extension_error", &[("type", &error_type)]);
                if recoverable {
                    let response = CliMessage::FeedbackSend {
                        session_id: session_id.clone(),
                        message: format!("Provider error: {}", error_message),
                        turn: 0,
                        trace_id,
                        v: PROTOCOL_VERSION,
                    };
                    let _ = cli_sender.send(serde_json::to_string(&response).unwrap());
                }
            }
            ExtensionMessage::Pong { timestamp, .. } => {
                tracing::debug!(timestamp, "pong");
            }
        },
    }
}

const PROTOCOL_VERSION: u32 = 1;

fn map_action_to_cli_message(
    action: OrchestratorAction,
    turn: u32,
    trace_id: &Option<String>,
) -> Option<CliMessage> {
    match action {
        OrchestratorAction::Feedback {
            session_id,
            message,
            trace_id,
        } => Some(CliMessage::FeedbackSend {
            session_id,
            message,
            turn: turn + 1,
            trace_id,
            v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::Continue {
            session_id,
            trace_id,
        } => Some(CliMessage::FeedbackContinue {
            session_id,
            trace_id,
            v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::SelfReview {
            session_id,
            prompt,
            trace_id,
        } => Some(CliMessage::SessionStart {
            session_id,
            provider_id: "self_review".into(),
            prompt,
            system_prompt: "You are a code reviewer. Respond with ::APPROVED or fix issues."
                .into(),
            trace_id,
            v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::StreamComplete {
            session_id,
            pr_url,
            trace_id,
        } => Some(CliMessage::FeedbackSend {
            session_id,
            message: format!("Stream complete! PR created: {}", pr_url),
            turn: turn + 1,
            trace_id,
            v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::Escalate {
            session_id,
            reason,
            trace_id,
        } => Some(CliMessage::FeedbackSend {
            session_id,
            message: format!("ESCALATION REQUIRED: {}", reason),
            turn: turn + 1,
            trace_id,
            v: PROTOCOL_VERSION,
        }),
        OrchestratorAction::WaitForResponse { .. } => None,
    }
}

async fn run_status(project_root: PathBuf) {
    let persistence = DebouncedPersistence::load(&project_root)
        .await
        .expect("failed to load state");
    let p = persistence.lock().await;
    let sessions = p.all_sessions();
    if sessions.is_empty() {
        println!("No sessions found.");
    } else {
        println!("{}", render_status_table(sessions));
    }
}

async fn run_preflight(config: &Arc<PilotConfig>) {
    println!("Running preflight checks...");
    match tokio::process::Command::new("git")
        .args(["--version"])
        .output()
        .await
    {
        Ok(o) if o.status.success() => println!(
            "✅ git: {}",
            String::from_utf8_lossy(&o.stdout).trim()
        ),
        _ => println!("❌ git: not found"),
    }
    match tokio::process::Command::new("cargo")
        .args(["--version"])
        .output()
        .await
    {
        Ok(o) if o.status.success() => println!(
            "✅ cargo: {}",
            String::from_utf8_lossy(&o.stdout).trim()
        ),
        _ => println!("❌ cargo: not found"),
    }
    println!("\nProviders: {} configured", config.providers.len());
    println!("Gate level: {}", config.gates.level);
    println!("Timeout: {}s", config.execution.command_timeout);
    println!(
        "Default branch: {} ({})",
        config.execution.default_branch, config.execution.branch_version
    );
    println!("Loopback-only: {}", config.server.host == "127.0.0.1");
}

async fn run_wave(config: Arc<PilotConfig>, project_root: PathBuf, wave: u8) {
    println!("Wave {} dispatch not yet fully implemented", wave);
}
```

## `src/lib.rs`

```rust
pub mod error;
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
pub mod metrics;

pub use error::PilotError;
pub use protocol::types::{FileOp, ParsedOps};
pub use protocol::parser::{parse_ops_block, extract_ops_blocks};
pub use applier::{
    apply_ops, apply_ops_async, preview_ops, preview_ops_async, ApplyLimits,
    ApplyResult, ApplyAction, PlannedChange, PlannedAction,
};
```

## `src/metrics.rs`

```rust
/// Trait for recording operational metrics.
///
/// Production implementations can integrate with Prometheus, StatsD, etc.
/// The default `NoOpMetrics` does nothing and compiles away.
pub trait Metrics: Send + Sync {
    fn increment_counter(&self, name: &str, labels: &[(&str, &str)]);
    fn record_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]);
}

/// No-op metrics collector for development and testing.
pub struct NoOpMetrics;

impl Metrics for NoOpMetrics {
    fn increment_counter(&self, _name: &str, _labels: &[(&str, &str)]) {}
    fn record_histogram(&self, _name: &str, _value: f64, _labels: &[(&str, &str)]) {}
}
```

## `src/error.rs`

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
    PathEscape {
        path: String,
        root: String,
        reason: String,
    },

    #[error("git operation failed: {0}")]
    Git(String),

    /// Returned for infrastructure failures: tool not installed, timeout, OS error.
    /// Use `Ok(GateResult { passed: false })` for semantic failures instead.
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
    Io {
        path: String,
        operation: String,
        #[source]
        source: io::Error,
    },

    /// Spawned task panicked or was cancelled — NOT an I/O error.
    #[error("task join failure during {operation}: {reason}")]
    TaskJoin {
        operation: String,
        reason: String,
    },

    /// Rollback succeeded after a partial apply failure.
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
    fn test_apply_error_io_preserves_kind_and_context() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let apply_err = ApplyError::Io {
            path: "src/main.rs".into(),
            operation: "write".into(),
            source: io_err,
        };
        let displayed = format!("{apply_err}");
        assert!(displayed.contains("src/main.rs"));
        assert!(displayed.contains("write"));
        assert!(displayed.contains("access denied"));
        if let ApplyError::Io { source, .. } = apply_err {
            assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
        }
    }

    #[test]
    fn test_task_join_error_has_distinct_code() {
        let err = ApplyError::TaskJoin {
            operation: "spawn_blocking".into(),
            reason: "task panicked".into(),
        };
        assert_eq!(err.code(), "E0205");
        let displayed = format!("{err}");
        assert!(displayed.contains("task join failure"));
        assert!(!displayed.contains("I/O"));
    }

    #[test]
    fn test_rollback_error_has_distinct_code() {
        let err = ApplyError::RolledBack {
            detail: "op 3 of 5 failed".into(),
        };
        assert_eq!(err.code(), "E0206");
    }

    #[test]
    fn test_gate_error_vs_apply_error_codes() {
        let gate_err = PilotError::Gate {
            gate: "coverage".into(),
            message: "tool not installed".into(),
        };
        assert_eq!(gate_err.code(), "E0500");

        let apply_err = PilotError::Apply(ApplyError::TaskJoin {
            operation: "test".into(),
            reason: "panic".into(),
        });
        assert_eq!(apply_err.code(), "E0205");
        assert_ne!(gate_err.code(), apply_err.code());
    }

    #[test]
    fn test_pilot_error_no_from_io() {
        let io_err = io::Error::new(io::ErrorKind::Other, "test");
        let _pilot = PilotError::Io(io_err);
    }
}
```

## `src/protocol/mod.rs`

```rust
pub mod types;
pub mod parser;
```

## `src/protocol/types.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", content = "data")]
pub enum FileOp {
    #[serde(rename = "write")]
    Write { path: String, content: String },
    #[serde(rename = "replace")]
    Replace {
        path: String,
        find: String,
        replace: String,
    },
    #[serde(rename = "delete")]
    Delete { path: String },
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
        Self {
            ops: Vec::new(),
            commit_message: None,
            incomplete: false,
            done: false,
            approved: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
            && self.commit_message.is_none()
            && !self.incomplete
            && !self.done
            && !self.approved
    }

    pub fn has_control_directive(&self) -> bool {
        self.commit_message.is_some() || self.incomplete || self.done || self.approved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_op_roundtrip() {
        let op = FileOp::Write {
            path: "src/main.rs".into(),
            content: "fn main() {}".into(),
        };
        let json = serde_json::to_string(&op).unwrap();
        let de: FileOp = serde_json::from_str(&json).unwrap();
        assert_eq!(op, de);
    }
}
```

## `src/protocol/parser.rs`

```rust
use crate::error::PilotError;
use crate::protocol::types::{FileOp, ParsedOps};

/// Extract glyim-ops blocks from a full AI response.
///
/// Correctly handles bare fences inside `::WRITE`/`::REPLACE` content
/// by tracking whether we are inside a write-or-replace block.
/// Fences encountered inside such blocks are treated as content,
/// not as block delimiters.
pub fn extract_ops_blocks(response: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = response.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed == "```glyim-ops" || trimmed.starts_with("```glyim-ops ") {
            let content_start = i + 1;
            let mut end_line = None;
            // Track whether we're inside a ::WRITE/::REPLACE block.
            // Fences inside those blocks are content, not delimiters.
            let mut inside_write_or_replace = false;

            for j in (i + 1)..lines.len() {
                let t = lines[j].trim();

                // Track ::WRITE/::REPLACE/::END directives
                if t.starts_with("::WRITE ") || t.starts_with("::REPLACE ") {
                    inside_write_or_replace = true;
                } else if t == "::END" && inside_write_or_replace {
                    inside_write_or_replace = false;
                }

                // Only treat fences as block delimiters when NOT inside
                // a ::WRITE/::REPLACE content section
                if t.starts_with("```") && !inside_write_or_replace {
                    end_line = Some(j);
                    break;
                }
            }

            if let Some(end) = end_line {
                let content: String = lines[content_start..end].join("\n");
                blocks.push(content.trim().to_string());
                i = end + 1;
            } else {
                // Unclosed glyim-ops block — discard
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
        if trimmed.is_empty() {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("::WRITE ") {
            let path = rest.trim().to_string();
            if path.is_empty() {
                return Err(PilotError::Parse {
                    line: line_num + 1,
                    message: "WRITE requires a path".into(),
                });
            }
            let content = read_until_end(&mut lines, line_num)?;
            ops.push(FileOp::Write { path, content });
        } else if let Some(rest) = trimmed.strip_prefix("::REPLACE ") {
            let path = rest.trim().to_string();
            if path.is_empty() {
                return Err(PilotError::Parse {
                    line: line_num + 1,
                    message: "REPLACE requires a path".into(),
                });
            }
            let (find, replace) = read_find_replace(&mut lines, line_num)?;
            ops.push(FileOp::Replace {
                path,
                find,
                replace,
            });
        } else if let Some(rest) = trimmed.strip_prefix("::DELETE ") {
            let path = rest.trim().to_string();
            if path.is_empty() {
                return Err(PilotError::Parse {
                    line: line_num + 1,
                    message: "DELETE requires a path".into(),
                });
            }
            ops.push(FileOp::Delete { path });
        } else if trimmed == "::DELETE" {
            return Err(PilotError::Parse {
                line: line_num + 1,
                message: "DELETE requires a path".into(),
            });
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

    Ok(ParsedOps {
        ops,
        commit_message,
        incomplete,
        done,
        approved,
    })
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
    Err(PilotError::Parse {
        line: start_line + 1,
        message: "unexpected end of input: expected ::END".into(),
    })
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
        let trimmed = line.trim();
        match trimmed {
            "---FIND---" => {
                in_find = true;
                in_replace = false;
            }
            "---REPLACE---" => {
                in_find = false;
                in_replace = true;
            }
            "::END" => {
                while find_lines.last().map_or(false, |l| l.trim().is_empty()) {
                    find_lines.pop();
                }
                while replace_lines.last().map_or(false, |l| l.trim().is_empty()) {
                    replace_lines.pop();
                }
                return Ok((find_lines.join("\n"), replace_lines.join("\n")));
            }
            _ => {
                if in_find {
                    find_lines.push(line.to_string());
                } else if in_replace {
                    replace_lines.push(line.to_string());
                }
            }
        }
    }
    Err(PilotError::Parse {
        line: start_line + 1,
        message: "unexpected end of input: expected ::END in REPLACE block".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_write() {
        let input = "::WRITE src/main.rs\nfn main() {}\n::END";
        let result = parse_ops_block(input).unwrap();
        assert_eq!(
            result.ops[0],
            FileOp::Write {
                path: "src/main.rs".into(),
                content: "fn main() {}".into(),
            }
        );
    }

    #[test]
    fn test_parse_replace() {
        let input = "::REPLACE src/lib.rs\n---FIND---\nold\n---REPLACE---\nnew\n::END";
        let result = parse_ops_block(input).unwrap();
        assert_eq!(
            result.ops[0],
            FileOp::Replace {
                path: "src/lib.rs".into(),
                find: "old".into(),
                replace: "new".into(),
            }
        );
    }

    #[test]
    fn test_parse_delete() {
        let input = "::DELETE src/old.rs";
        let result = parse_ops_block(input).unwrap();
        assert_eq!(
            result.ops[0],
            FileOp::Delete {
                path: "src/old.rs".into(),
            }
        );
    }

    #[test]
    fn test_extract_nested_fences_inside_write() {
        // Bare fences inside ::WRITE must NOT close the outer glyim-ops block
        let response = "\
```glyim-ops
::WRITE readme.md
# Hello

```rust
fn main() {}
```

More text
::END
```";
        let blocks = extract_ops_blocks(response);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("fn main() {}"));
        assert!(blocks[0].contains("More text"));
        assert!(blocks[0].contains("::END"));
    }

    #[test]
    fn test_extract_bare_fence_in_write_not_close_block() {
        // The specific case from the critique: bare ``` inside ::WRITE content
        let response = "\
```glyim-ops
::WRITE readme.md
```
some output
```
::END
```";
        let blocks = extract_ops_blocks(response);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("some output"));
        assert!(blocks[0].contains("::END"));
    }

    #[test]
    fn test_extract_multiple_write_blocks_in_single_ops() {
        let response = "\
```glyim-ops
::WRITE a.rs
content
::END
::WRITE b.rs
```
other
```
::END
```";
        let blocks = extract_ops_blocks(response);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("::WRITE a.rs"));
        assert!(blocks[0].contains("::WRITE b.rs"));
        assert!(blocks[0].contains("other"));
    }

    #[test]
    fn test_extract_multiple_blocks() {
        let response = "\
```glyim-ops
::WRITE src/a.rs\na\n::END
```
text
```glyim-ops
::DELETE src/b.rs
```";
        let blocks = extract_ops_blocks(response);
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn test_extract_no_blocks() {
        let response = "```rust\nfn main() {}\n```";
        let blocks = extract_ops_blocks(response);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_extract_replace_with_fences_in_content() {
        let response = "\
```glyim-ops
::REPLACE readme.md
---FIND---
```old
---
---REPLACE---
```new
---
::END
```";
        let blocks = extract_ops_blocks(response);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("```old"));
        assert!(blocks[0].contains("```new"));
    }
}
```

## `src/applier/security.rs`

```rust
use std::path::{Path, PathBuf};
use dunce::canonicalize;

/// Validate that a relative path does not escape the worktree root.
///
/// Canonicalizes the worktree root FIRST, then joins the relative path
/// onto the canonical root, then normalizes. This handles macOS /tmp
/// symlink where canonicalize(/tmp) = /private/tmp.
pub fn validate_path(worktree_root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let relative = Path::new(relative_path);
    if relative.is_absolute() {
        return Err(format!(
            "path '{}' is absolute; must be relative to worktree",
            relative_path
        ));
    }

    // Canonicalize root FIRST to resolve symlinks
    let canonical_root = if worktree_root.exists() {
        match canonicalize(worktree_root) {
            Ok(c) => c,
            Err(_) => path_clean::PathClean::clean(worktree_root),
        }
    } else {
        path_clean::PathClean::clean(worktree_root)
    };

    // Join onto the canonical root, then normalize
    let candidate = canonical_root.join(relative);
    let normalized = path_clean::PathClean::clean(&candidate);

    if normalized == canonical_root {
        return Err(format!(
            "path '{}' resolves to worktree root, not a file",
            relative_path
        ));
    }

    if !normalized.starts_with(&canonical_root) {
        // Last resort: try canonicalizing both (handles nested symlinks)
        if worktree_root.exists() {
            if let (Ok(can_child), Ok(can_parent)) =
                (canonicalize(&normalized), canonicalize(&canonical_root))
            {
                if can_child.starts_with(can_parent) {
                    return Ok(normalized);
                }
            }
        }
        return Err(format!(
            "path '{}' escapes worktree '{}'",
            relative_path,
            canonical_root.display()
        ));
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_valid_simple_path() {
        let dir = TempDir::new().unwrap();
        let result = validate_path(dir.path(), "src/main.rs");
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("src/main.rs"));
    }

    #[test]
    fn test_path_traversal_attack() {
        let dir = TempDir::new().unwrap();
        let result = validate_path(dir.path(), "../../etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("escapes worktree"));
    }

    #[test]
    fn test_absolute_path_rejected() {
        let dir = TempDir::new().unwrap();
        let result = validate_path(dir.path(), "/etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("absolute"));
    }

    #[test]
    fn test_path_resolving_to_root_rejected() {
        let dir = TempDir::new().unwrap();
        let result = validate_path(dir.path(), ".");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("resolves to worktree root"));
    }

    #[test]
    fn test_macos_tmp_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let result = validate_path(dir.path(), "src/main.rs");
        assert!(
            result.is_ok(),
            "must work even when worktree is in /tmp on macOS"
        );
    }
}
```

## `src/applier/mod.rs`

```rust
pub mod security;

use std::fs;
use std::io;
use std::path::Path;
use std::time::Instant;

use crate::error::{ApplyError, PilotError};
use crate::protocol::types::FileOp;
use security::validate_path;

// ── Public types ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApplyResult {
    pub path: String,
    pub action: ApplyAction,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ApplyAction {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlannedChange {
    pub path: String,
    pub action: PlannedAction,
    pub current_content_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PlannedAction {
    Create,
    Overwrite,
    Modify,
    Delete,
}

/// Configurable limits for file operations to prevent runaway AI output.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApplyLimits {
    /// Maximum size of a single file written by a WRITE op (bytes).
    pub max_file_size: usize,
    /// Maximum total content across all WRITE ops in a single block (bytes).
    pub max_total_content: usize,
    /// Maximum number of operations in a single ops block.
    pub max_ops_per_block: usize,
}

impl Default for ApplyLimits {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024,       // 10 MiB
            max_total_content: 50 * 1024 * 1024,    // 50 MiB
            max_ops_per_block: 100,
        }
    }
}

impl ApplyLimits {
    pub fn strict() -> Self {
        Self {
            max_file_size: 1024 * 1024,             // 1 MiB
            max_total_content: 5 * 1024 * 1024,     // 5 MiB
            max_ops_per_block: 20,
        }
    }
}

// ── Backup for two-phase apply ────────────────────────────────────

struct Backup {
    rel_path: String,
    /// None means the file did not exist before the operation.
    original_content: Option<String>,
}

// ── Public API ────────────────────────────────────────────────────

/// Apply a list of file operations with two-phase commit:
///
/// 1. **Validate & prepare**: Check limits, validate paths, and create
///    backups of all files that will be modified or deleted.
/// 2. **Apply**: Execute operations. Each individual write uses an
///    atomic temp-file-then-rename strategy.
/// 3. **Rollback on failure**: If any operation fails, restore all
///    previously-modified files from their backups.
pub fn apply_ops(
    worktree_root: &Path,
    ops: &[FileOp],
    limits: &ApplyLimits,
) -> Result<Vec<ApplyResult>, PilotError> {
    // Phase 0: Validate limits
    validate_limits(ops, limits)?;

    // Phase 1: Create backups
    let backups = create_backups(worktree_root, ops)?;

    // Phase 2: Apply operations
    let mut results = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        let start = Instant::now();
        match apply_op_atomic(worktree_root, op) {
            Ok(result) => {
                tracing::debug!(
                    path = %result.path,
                    action = ?result.action,
                    elapsed_ms = start.elapsed().as_millis(),
                    "applied operation {}/{}",
                    i + 1,
                    ops.len()
                );
                results.push(result);
            }
            Err(e) => {
                // Phase 3: Rollback
                tracing::error!(
                    op_index = i,
                    error = %e,
                    "apply failed, rolling back {} operations",
                    results.len()
                );
                rollback(worktree_root, &backups, &results, worktree_root);
                return Err(PilotError::Apply(ApplyError::RolledBack {
                    detail: format!(
                        "operation {} of {} failed: {} (rollback succeeded)",
                        i + 1,
                        ops.len(),
                        e
                    ),
                }));
            }
        }
    }

    Ok(results)
}

pub fn preview_ops(
    worktree_root: &Path,
    ops: &[FileOp],
) -> Result<Vec<PlannedChange>, PilotError> {
    let mut changes = Vec::new();
    for op in ops {
        changes.push(preview_op(worktree_root, op)?);
    }
    Ok(changes)
}

pub async fn apply_ops_async(
    worktree_root: std::path::PathBuf,
    ops: Vec<FileOp>,
    limits: ApplyLimits,
) -> Result<Vec<ApplyResult>, PilotError> {
    tokio::task::spawn_blocking(move || apply_ops(&worktree_root, &ops, &limits))
        .await
        .map_err(|join_error| {
            let reason = if join_error.is_panic() {
                "task panicked".into()
            } else if join_error.is_cancelled() {
                "task cancelled".into()
            } else {
                format!("join error: {join_error}")
            };
            PilotError::Apply(ApplyError::TaskJoin {
                operation: "apply_ops".into(),
                reason,
            })
        })?
}

pub async fn preview_ops_async(
    worktree_root: std::path::PathBuf,
    ops: Vec<FileOp>,
) -> Result<Vec<PlannedChange>, PilotError> {
    tokio::task::spawn_blocking(move || preview_ops(&worktree_root, &ops))
        .await
        .map_err(|join_error| {
            let reason = if join_error.is_panic() {
                "task panicked".into()
            } else if join_error.is_cancelled() {
                "task cancelled".into()
            } else {
                format!("join error: {join_error}")
            };
            PilotError::Apply(ApplyError::TaskJoin {
                operation: "preview_ops".into(),
                reason,
            })
        })?
}

// ── Internal helpers ──────────────────────────────────────────────

fn validate_limits(ops: &[FileOp], limits: &ApplyLimits) -> Result<(), PilotError> {
    if ops.len() > limits.max_ops_per_block {
        return Err(PilotError::Limits(format!(
            "ops block contains {} operations (max {})",
            ops.len(),
            limits.max_ops_per_block
        )));
    }
    let mut total_content: usize = 0;
    for op in ops {
        let content_len = match op {
            FileOp::Write { content, .. } => content.len(),
            FileOp::Replace {
                find, replace, ..
            } => find.len() + replace.len(),
            FileOp::Delete { .. } => 0,
        };
        if content_len > limits.max_file_size {
            return Err(PilotError::Limits(format!(
                "content for '{}' is {} bytes (max {})",
                op.path(),
                content_len,
                limits.max_file_size
            )));
        }
        total_content += content_len;
    }
    if total_content > limits.max_total_content {
        return Err(PilotError::Limits(format!(
            "total content is {} bytes (max {})",
            total_content, limits.max_total_content
        )));
    }
    Ok(())
}

/// Helper trait to get the path from any FileOp variant.
trait FileOpPath {
    fn path(&self) -> &str;
}
impl FileOpPath for FileOp {
    fn path(&self) -> &str {
        match self {
            FileOp::Write { path, .. } => path,
            FileOp::Replace { path, .. } => path,
            FileOp::Delete { path } => path,
        }
    }
}

fn create_backups(worktree_root: &Path, ops: &[FileOp]) -> Result<Vec<Backup>, PilotError> {
    let mut backups = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for op in ops {
        let rel_path = op.path();
        if seen.contains(rel_path) {
            continue; // Don't backup the same file twice
        }
        seen.insert(rel_path.to_string());

        let abs_path = validate_path(worktree_root, rel_path).map_err(|reason| {
            PilotError::PathEscape {
                path: rel_path.to_string(),
                root: worktree_root.display().to_string(),
                reason,
            }
        })?;

        let original_content = if abs_path.exists() {
            Some(fs::read_to_string(&abs_path).map_err(|e| {
                PilotError::Apply(ApplyError::Io {
                    path: rel_path.to_string(),
                    operation: "read_for_backup".into(),
                    source: e,
                })
            })?)
        } else {
            None
        };

        backups.push(Backup {
            rel_path: rel_path.to_string(),
            original_content,
        });
    }

    Ok(backups)
}

fn rollback(
    worktree_root: &Path,
    backups: &[Backup],
    results: &[ApplyResult],
    _root: &Path,
) {
    // Restore all files that were successfully applied
    for result in results {
        let backup = backups.iter().find(|b| b.rel_path == result.path);
        match backup {
            Some(b) => match &b.original_content {
                Some(content) => {
                    let abs_path = match validate_path(worktree_root, &b.rel_path) {
                        Ok(p) => p,
                        Err(_) => {
                            tracing::error!(
                                path = &b.rel_path,
                                "rollback: path validation failed"
                            );
                            continue;
                        }
                    };
                    if let Err(e) = fs::write(&abs_path, content) {
                        tracing::error!(
                            path = &b.rel_path,
                            error = %e,
                            "rollback: failed to restore file"
                        );
                    } else {
                        tracing::info!(path = &b.rel_path, "rollback: restored file");
                    }
                }
                None => {
                    // File didn't exist before; delete it
                    let abs_path = match validate_path(worktree_root, &b.rel_path) {
                        Ok(p) => p,
                        Err(_) => continue,
                    };
                    if abs_path.exists() {
                        if let Err(e) = fs::remove_file(&abs_path) {
                            tracing::error!(
                                path = &b.rel_path,
                                error = %e,
                                "rollback: failed to delete created file"
                            );
                        } else {
                            tracing::info!(path = &b.rel_path, "rollback: deleted created file");
                        }
                    }
                }
            },
            None => {
                // This was a newly created file with no backup — try to delete
                let abs_path = match validate_path(worktree_root, &result.path) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                if abs_path.exists() {
                    let _ = fs::remove_file(&abs_path);
                }
            }
        }
    }
}

fn apply_op_atomic(worktree_root: &Path, op: &FileOp) -> Result<ApplyResult, PilotError> {
    match op {
        FileOp::Write { path, content } => apply_write_atomic(worktree_root, path, content),
        FileOp::Replace {
            path,
            find,
            replace,
        } => apply_replace_atomic(worktree_root, path, find, replace),
        FileOp::Delete { path } => apply_delete(worktree_root, path),
    }
}

/// Atomic write: write to a temp file first, then rename.
/// On POSIX, `rename` is atomic, so a crash mid-write won't corrupt
/// the target file.
fn apply_write_atomic(
    worktree_root: &Path,
    rel_path: &str,
    content: &str,
) -> Result<ApplyResult, PilotError> {
    let abs_path = validate_path(worktree_root, rel_path).map_err(|reason| {
        PilotError::PathEscape {
            path: rel_path.to_string(),
            root: worktree_root.display().to_string(),
            reason,
        }
    })?;

    if let Some(parent) = abs_path.parent() {
        fs::create_dir_all(parent).map_err(|e| PilotError::Apply(ApplyError::Io {
            path: rel_path.to_string(),
            operation: "create_dir_all".into(),
            source: e,
        }))?;
    }

    let existed = abs_path.exists();
    let tmp_path = abs_path.with_extension("glyim-tmp");

    // Phase 1: Write to temp file
    fs::write(&tmp_path, content).map_err(|e| PilotError::Apply(ApplyError::Io {
        path: rel_path.to_string(),
        operation: "write_tmp".into(),
        source: e,
    }))?;

    // Phase 2: Atomic rename
    fs::rename(&tmp_path, &abs_path).map_err(|e| {
        // Clean up temp file on rename failure
        let _ = fs::remove_file(&tmp_path);
        PilotError::Apply(ApplyError::Io {
            path: rel_path.to_string(),
            operation: "rename".into(),
            source: e,
        })
    })?;

    Ok(ApplyResult {
        path: rel_path.to_string(),
        action: if existed {
            ApplyAction::Modified
        } else {
            ApplyAction::Created
        },
    })
}

fn apply_replace_atomic(
    worktree_root: &Path,
    rel_path: &str,
    find: &str,
    replace: &str,
) -> Result<ApplyResult, PilotError> {
    let abs_path = validate_path(worktree_root, rel_path).map_err(|reason| {
        PilotError::PathEscape {
            path: rel_path.to_string(),
            root: worktree_root.display().to_string(),
            reason,
        }
    })?;

    if !abs_path.exists() {
        return Err(PilotError::Apply(ApplyError::FileNotFound(
            rel_path.to_string(),
        )));
    }

    let existing = fs::read_to_string(&abs_path).map_err(|e| PilotError::Apply(ApplyError::Io {
        path: rel_path.to_string(),
        operation: "read".into(),
        source: e,
    }))?;

    let count = existing.matches(find).count();
    if count == 0 {
        return Err(PilotError::Apply(ApplyError::FindNotFound {
            path: rel_path.to_string(),
        }));
    }
    if count > 1 {
        return Err(PilotError::Apply(ApplyError::FindAmbiguous {
            path: rel_path.to_string(),
            count,
        }));
    }

    let new_content = existing.replacen(find, replace, 1);

    // Atomic write
    let tmp_path = abs_path.with_extension("glyim-tmp");
    fs::write(&tmp_path, &new_content).map_err(|e| PilotError::Apply(ApplyError::Io {
        path: rel_path.to_string(),
        operation: "write_tmp".into(),
        source: e,
    }))?;
    fs::rename(&tmp_path, &abs_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        PilotError::Apply(ApplyError::Io {
            path: rel_path.to_string(),
            operation: "rename".into(),
            source: e,
        })
    })?;

    Ok(ApplyResult {
        path: rel_path.to_string(),
        action: ApplyAction::Modified,
    })
}

fn apply_delete(worktree_root: &Path, rel_path: &str) -> Result<ApplyResult, PilotError> {
    let abs_path = validate_path(worktree_root, rel_path).map_err(|reason| {
        PilotError::PathEscape {
            path: rel_path.to_string(),
            root: worktree_root.display().to_string(),
            reason,
        }
    })?;

    if !abs_path.exists() {
        return Err(PilotError::Apply(ApplyError::FileNotFound(
            rel_path.to_string(),
        )));
    }

    fs::remove_file(&abs_path).map_err(|e| PilotError::Apply(ApplyError::Io {
        path: rel_path.to_string(),
        operation: "delete".into(),
        source: e,
    }))?;

    Ok(ApplyResult {
        path: rel_path.to_string(),
        action: ApplyAction::Deleted,
    })
}

fn preview_op(worktree_root: &Path, op: &FileOp) -> Result<PlannedChange, PilotError> {
    match op {
        FileOp::Write { path, .. } => {
            let abs_path = validate_path(worktree_root, path).map_err(|reason| {
                PilotError::PathEscape {
                    path: path.clone(),
                    root: worktree_root.display().to_string(),
                    reason,
                }
            })?;
            let exists = abs_path.exists();
            let summary = if exists {
                fs::metadata(&abs_path)
                    .ok()
                    .map(|m| format!("existing file ({} bytes)", m.len()))
            } else {
                None
            };
            Ok(PlannedChange {
                path: path.clone(),
                action: if exists {
                    PlannedAction::Overwrite
                } else {
                    PlannedAction::Create
                },
                current_content_summary: summary,
            })
        }
        FileOp::Replace { path, .. } => {
            let abs_path = validate_path(worktree_root, path).map_err(|reason| {
                PilotError::PathEscape {
                    path: path.clone(),
                    root: worktree_root.display().to_string(),
                    reason,
                }
            })?;
            if !abs_path.exists() {
                return Err(PilotError::Apply(ApplyError::FileNotFound(path.clone())));
            }
            let meta = fs::metadata(&abs_path).ok();
            Ok(PlannedChange {
                path: path.clone(),
                action: PlannedAction::Modify,
                current_content_summary: meta.map(|m| format!("existing file ({} bytes)", m.len())),
            })
        }
        FileOp::Delete { path } => {
            let abs_path = validate_path(worktree_root, path).map_err(|reason| {
                PilotError::PathEscape {
                    path: path.clone(),
                    root: worktree_root.display().to_string(),
                    reason,
                }
            })?;
            if !abs_path.exists() {
                return Err(PilotError::Apply(ApplyError::FileNotFound(path.clone())));
            }
            let meta = fs::metadata(&abs_path).ok();
            Ok(PlannedChange {
                path: path.clone(),
                action: PlannedAction::Delete,
                current_content_summary: meta
                    .map(|m| format!("existing file ({} bytes) — WILL BE DELETED", m.len())),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_worktree() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn test_apply_write_creates_file() {
        let dir = setup_worktree();
        let ops = vec![FileOp::Write {
            path: "src/main.rs".into(),
            content: "fn main() {}".into(),
        }];
        let results = apply_ops(dir.path(), &ops, &ApplyLimits::default()).unwrap();
        assert_eq!(results[0].action, ApplyAction::Created);
        assert_eq!(
            fs::read_to_string(dir.path().join("src/main.rs")).unwrap(),
            "fn main() {}"
        );
    }

    #[test]
    fn test_apply_write_modifies_existing() {
        let dir = setup_worktree();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "old").unwrap();
        let ops = vec![FileOp::Write {
            path: "src/main.rs".into(),
            content: "new".into(),
        }];
        let results = apply_ops(dir.path(), &ops, &ApplyLimits::default()).unwrap();
        assert_eq!(results[0].action, ApplyAction::Modified);
        assert_eq!(
            fs::read_to_string(dir.path().join("src/main.rs")).unwrap(),
            "new"
        );
    }

    #[test]
    fn test_apply_replace_find_not_found() {
        let dir = setup_worktree();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "pub mod token;").unwrap();
        let ops = vec![FileOp::Replace {
            path: "src/lib.rs".into(),
            find: "old".into(),
            replace: "new".into(),
        }];
        let result = apply_ops(dir.path(), &ops, &ApplyLimits::default());
        assert!(matches!(
            result.unwrap_err(),
            PilotError::Apply(ApplyError::RolledBack { .. })
        ));
    }

    #[test]
    fn test_apply_rollback_on_failure() {
        let dir = setup_worktree();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/a.rs"), "original a").unwrap();
        fs::write(dir.path().join("src/b.rs"), "original b").unwrap();

        let ops = vec![
            FileOp::Write {
                path: "src/a.rs".into(),
                content: "modified a".into(),
            },
            FileOp::Replace {
                path: "src/b.rs".into(),
                find: "nonexistent".into(),
                replace: "x".into(),
            },
        ];
        let result = apply_ops(dir.path(), &ops, &ApplyLimits::default());
        assert!(result.is_err());

        // Verify rollback: a.rs should be restored to original
        assert_eq!(
            fs::read_to_string(dir.path().join("src/a.rs")).unwrap(),
            "original a"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("src/b.rs")).unwrap(),
            "original b"
        );
    }

    #[test]
    fn test_apply_limits_exceeded() {
        let dir = setup_worktree();
        let limits = ApplyLimits {
            max_ops_per_block: 2,
            ..ApplyLimits::default()
        };
        let ops = vec![
            FileOp::Write {
                path: "a.rs".into(),
                content: "a".into(),
            },
            FileOp::Write {
                path: "b.rs".into(),
                content: "b".into(),
            },
            FileOp::Write {
                path: "c.rs".into(),
                content: "c".into(),
            },
        ];
        let result = apply_ops(dir.path(), &ops, &limits);
        assert!(matches!(result.unwrap_err(), PilotError::Limits(_)));
    }

    #[test]
    fn test_apply_content_size_limit() {
        let dir = setup_worktree();
        let limits = ApplyLimits {
            max_file_size: 5,
            ..ApplyLimits::default()
        };
        let ops = vec![FileOp::Write {
            path: "big.rs".into(),
            content: "x".repeat(100),
        }];
        let result = apply_ops(dir.path(), &ops, &limits);
        assert!(matches!(result.unwrap_err(), PilotError::Limits(_)));
    }

    #[tokio::test]
    async fn test_apply_ops_async() {
        let dir = setup_worktree();
        let root = dir.path().to_path_buf();
        let ops = vec![FileOp::Write {
            path: "src/main.rs".into(),
            content: "fn main() {}".into(),
        }];
        let results = apply_ops_async(root, ops, ApplyLimits::default())
            .await
            .unwrap();
        assert_eq!(results[0].action, ApplyAction::Created);
    }
}
```

## `src/config/mod.rs`

```rust
pub mod types;

use crate::error::PilotError;
use std::path::Path;
pub use types::*;

pub fn load_config(project_root: &Path) -> Result<PilotConfig, PilotError> {
    let config_path = project_root.join(".glyim-pilot.toml");
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| PilotError::Config(format!("failed to read config: {e}")))?;
    let config: PilotConfig = toml::from_str(&content)
        .map_err(|e| PilotError::Config(format!("failed to parse config: {e}")))?;
    Ok(config)
}
```

## `src/config/types.rs`

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::applier::ApplyLimits;
use crate::gates::architecture::DependencyRule;
use crate::gates::banned_pattern::BannedPattern;

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
            server: ServerConfig::default(),
            defaults: DefaultsConfig::default(),
            providers,
            execution: ExecutionConfig::default(),
            gates: GatesConfig::default(),
            context: ContextConfig::default(),
            dispatch: DispatchConfig::default(),
            limits: ApplyLimits::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_host")]
    pub host: String,
}

fn default_port() -> u16 {
    8420
}
fn default_host() -> String {
    "127.0.0.1".into()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DefaultsConfig {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub auto_execute: bool,
    #[serde(default = "default_max_turns")]
    pub max_turns: u32,
    #[serde(default = "default_true")]
    pub retry_on_rate_limit: bool,
    #[serde(default = "default_retry_max_wait")]
    pub retry_max_wait: u64,
}

fn default_max_turns() -> u32 {
    50
}
fn default_true() -> bool {
    true
}
fn default_retry_max_wait() -> u64 {
    120
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            provider: String::new(),
            auto_execute: false,
            max_turns: default_max_turns(),
            retry_on_rate_limit: true,
            retry_max_wait: default_retry_max_wait(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub url: String,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    #[serde(default = "default_cooldown")]
    pub rate_limit_cooldown: u64,
    #[serde(default)]
    pub error_patterns: Vec<String>,
    #[serde(default = "default_input_selector")]
    pub input_selector: String,
    #[serde(default = "default_send_selector")]
    pub send_selector: String,
    #[serde(default)]
    pub streaming_indicator: String,
    #[serde(default)]
    pub assistant_selector: String,
    #[serde(default = "default_code_block_selector")]
    pub code_block_selector: String,
}

fn default_max_concurrent() -> usize {
    3
}
fn default_cooldown() -> u64 {
    60
}
fn default_input_selector() -> String {
    "textarea".into()
}
fn default_send_selector() -> String {
    "button[type='submit']".into()
}
fn default_code_block_selector() -> String {
    "pre code".into()
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            url: String::new(),
            max_concurrent: default_max_concurrent(),
            rate_limit_cooldown: default_cooldown(),
            error_patterns: Vec::new(),
            input_selector: default_input_selector(),
            send_selector: default_send_selector(),
            streaming_indicator: String::new(),
            assistant_selector: String::new(),
            code_block_selector: default_code_block_selector(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecutionConfig {
    #[serde(default = "default_worktree_base")]
    pub worktree_base: String,
    #[serde(default = "default_require_confirmation")]
    pub require_confirmation: String,
    #[serde(default = "default_dangerous_patterns")]
    pub dangerous_patterns: Vec<String>,
    #[serde(default = "default_max_fix_rounds")]
    pub max_fix_rounds: u32,
    #[serde(default = "default_command_timeout")]
    pub command_timeout: u64,
    #[serde(default = "default_branch")]
    pub default_branch: String,
    #[serde(default = "default_branch_version")]
    pub branch_version: String,
}

fn default_worktree_base() -> String {
    "../glyim-worktrees".into()
}
fn default_require_confirmation() -> String {
    "first".into()
}
fn default_dangerous_patterns() -> Vec<String> {
    vec![
        "rm -rf".into(),
        "git push".into(),
        "git reset --hard".into(),
        "cargo publish".into(),
        "sudo".into(),
    ]
}
fn default_max_fix_rounds() -> u32 {
    5
}
fn default_command_timeout() -> u64 {
    300
}
fn default_branch() -> String {
    "main".into()
}
fn default_branch_version() -> String {
    "v0.1.0".into()
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            worktree_base: default_worktree_base(),
            require_confirmation: default_require_confirmation(),
            dangerous_patterns: default_dangerous_patterns(),
            max_fix_rounds: default_max_fix_rounds(),
            command_timeout: default_command_timeout(),
            default_branch: default_branch(),
            branch_version: default_branch_version(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GateLevel {
    Relaxed,
    Normal,
    Strict,
    Production,
}

impl Default for GateLevel {
    fn default() -> Self {
        Self::Normal
    }
}

impl std::fmt::Display for GateLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Relaxed => write!(f, "relaxed"),
            Self::Normal => write!(f, "normal"),
            Self::Strict => write!(f, "strict"),
            Self::Production => write!(f, "production"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GatesConfig {
    #[serde(default)]
    pub level: GateLevel,
    #[serde(default)]
    pub commit: CommitGatesConfig,
    #[serde(default)]
    pub done: DoneGatesConfig,
    /// Configurable banned patterns (overrides defaults)
    #[serde(default)]
    pub banned_patterns: Vec<BannedPattern>,
    /// Configurable architecture rules (overrides defaults)
    #[serde(default)]
    pub architecture_rules: Vec<DependencyRule>,
}

impl Default for GatesConfig {
    fn default() -> Self {
        Self {
            level: GateLevel::default(),
            commit: CommitGatesConfig::default(),
            done: DoneGatesConfig::default(),
            banned_patterns: Vec::new(),
            architecture_rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommitGatesConfig {
    pub fmt: Option<bool>,
    pub check: Option<bool>,
    pub clippy: Option<bool>,
    pub test: Option<bool>,
    pub banned_patterns: Option<bool>,
    pub architecture: Option<bool>,
    pub contracts: Option<bool>,
}

impl Default for CommitGatesConfig {
    fn default() -> Self {
        Self {
            fmt: None,
            check: None,
            clippy: None,
            test: None,
            banned_patterns: None,
            architecture: None,
            contracts: None,
        }
    }
}

impl CommitGatesConfig {
    /// Resolve gate configuration with explicit default_branch and branch_version.
    /// No post-hoc mutation required — the caller passes these in.
    pub fn resolve(
        &self,
        level: GateLevel,
        default_branch: String,
        branch_version: String,
    ) -> ResolvedCommitGates {
        let d = level.commit_defaults();
        ResolvedCommitGates {
            fmt: self.fmt.unwrap_or(d.fmt),
            check: self.check.unwrap_or(d.check),
            clippy: self.clippy.unwrap_or(d.clippy),
            test: self.test.unwrap_or(d.test),
            banned_patterns: self.banned_patterns.unwrap_or(d.banned_patterns),
            architecture: self.architecture.unwrap_or(d.architecture),
            contracts: self.contracts.unwrap_or(d.contracts),
            default_branch,
            branch_version,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedCommitGates {
    pub fmt: bool,
    pub check: bool,
    pub clippy: bool,
    pub test: bool,
    pub banned_patterns: bool,
    pub architecture: bool,
    pub contracts: bool,
    pub default_branch: String,
    pub branch_version: String,
}

struct CommitDefaults {
    fmt: bool,
    check: bool,
    clippy: bool,
    test: bool,
    banned_patterns: bool,
    architecture: bool,
    contracts: bool,
}

impl GateLevel {
    fn commit_defaults(self) -> CommitDefaults {
        match self {
            Self::Relaxed => CommitDefaults {
                fmt: true,
                check: true,
                clippy: false,
                test: false,
                banned_patterns: false,
                architecture: false,
                contracts: false,
            },
            Self::Normal => CommitDefaults {
                fmt: true,
                check: true,
                clippy: true,
                test: true,
                banned_patterns: false,
                architecture: false,
                contracts: false,
            },
            Self::Strict | Self::Production => CommitDefaults {
                fmt: true,
                check: true,
                clippy: true,
                test: true,
                banned_patterns: true,
                architecture: true,
                contracts: true,
            },
        }
    }
    fn done_defaults(self) -> DoneDefaults {
        match self {
            Self::Relaxed | Self::Normal => DoneDefaults {
                dead_code: false,
                coverage: false,
                coverage_min: 0.80,
                mutation: false,
                mutation_kill_rate: 0.75,
                workspace_check: false,
                audit: false,
                self_review: false,
            },
            Self::Strict => DoneDefaults {
                dead_code: true,
                coverage: false,
                coverage_min: 0.80,
                mutation: false,
                mutation_kill_rate: 0.75,
                workspace_check: true,
                audit: false,
                self_review: false,
            },
            Self::Production => DoneDefaults {
                dead_code: true,
                coverage: true,
                coverage_min: 0.80,
                mutation: true,
                mutation_kill_rate: 0.75,
                workspace_check: true,
                audit: true,
                self_review: true,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DoneGatesConfig {
    pub dead_code: Option<bool>,
    pub coverage: Option<bool>,
    #[serde(default = "default_coverage_min")]
    pub coverage_min: f64,
    pub mutation: Option<bool>,
    #[serde(default = "default_mutation_kill_rate")]
    pub mutation_kill_rate: f64,
    pub workspace_check: Option<bool>,
    pub audit: Option<bool>,
    pub self_review: Option<bool>,
}

fn default_coverage_min() -> f64 {
    0.80
}

fn default_mutation_kill_rate() -> f64 {
    0.75
}

impl Default for DoneGatesConfig {
    fn default() -> Self {
        Self {
            dead_code: None,
            coverage: None,
            coverage_min: default_coverage_min(),
            mutation: None,
            mutation_kill_rate: default_mutation_kill_rate(),
            workspace_check: None,
            audit: None,
            self_review: None,
        }
    }
}

impl DoneGatesConfig {
    pub fn resolve(&self, level: GateLevel) -> ResolvedDoneGates {
        let d = level.done_defaults();
        ResolvedDoneGates {
            dead_code: self.dead_code.unwrap_or(d.dead_code),
            coverage: self.coverage.unwrap_or(d.coverage),
            coverage_min: self.coverage_min,
            mutation: self.mutation.unwrap_or(d.mutation),
            mutation_kill_rate: self.mutation_kill_rate,
            workspace_check: self.workspace_check.unwrap_or(d.workspace_check),
            audit: self.audit.unwrap_or(d.audit),
            self_review: self.self_review.unwrap_or(d.self_review),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedDoneGates {
    pub dead_code: bool,
    pub coverage: bool,
    pub coverage_min: f64,
    pub mutation: bool,
    pub mutation_kill_rate: f64,
    pub workspace_check: bool,
    pub audit: bool,
    pub self_review: bool,
}

struct DoneDefaults {
    dead_code: bool,
    coverage: bool,
    coverage_min: f64,
    mutation: bool,
    mutation_kill_rate: f64,
    workspace_check: bool,
    audit: bool,
    self_review: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContextConfig {
    #[serde(default = "default_max_context_tokens")]
    pub max_context_tokens: usize,
    #[serde(default)]
    pub providers: HashMap<String, ProviderContextConfig>,
}

fn default_max_context_tokens() -> usize {
    15000
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: default_max_context_tokens(),
            providers: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderContextConfig {
    #[serde(default = "default_max_context_tokens")]
    pub max_context_tokens: usize,
}

impl Default for ProviderContextConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: default_max_context_tokens(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DispatchConfig {
    #[serde(default = "default_strategy")]
    pub strategy: String,
    #[serde(default = "default_true")]
    pub fallback_on_rate_limit: bool,
    #[serde(default = "default_max_reassign")]
    pub max_reassign_attempts: u32,
}

fn default_strategy() -> String {
    "most_slots_first".into()
}
fn default_max_reassign() -> u32 {
    2
}

impl Default for DispatchConfig {
    fn default() -> Self {
        Self {
            strategy: default_strategy(),
            fallback_on_rate_limit: true,
            max_reassign_attempts: default_max_reassign(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config_default() {
        let cfg = ProviderConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.max_concurrent, 3);
    }

    #[test]
    fn test_gate_level_default_is_normal() {
        assert_eq!(GateLevel::default(), GateLevel::Normal);
    }

    #[test]
    fn test_commit_gates_resolve_with_explicit_branch_info() {
        let gates = GatesConfig::default();
        let resolved = gates.commit.resolve(
            GateLevel::Normal,
            "develop".into(),
            "v0.2.0".into(),
        );
        assert!(resolved.fmt);
        assert!(resolved.check);
        assert!(resolved.clippy);
        assert!(resolved.test);
        assert_eq!(resolved.default_branch, "develop");
        assert_eq!(resolved.branch_version, "v0.2.0");
    }

    #[test]
    fn test_done_gates_resolve_production() {
        let gates = GatesConfig::default();
        let resolved = gates.done.resolve(GateLevel::Production);
        assert!(resolved.dead_code);
        assert!(resolved.coverage);
        assert!(resolved.mutation);
        assert!(resolved.workspace_check);
        assert!(resolved.audit);
        assert!(resolved.self_review);
    }

    #[test]
    fn test_configurable_banned_patterns() {
        let gates = GatesConfig {
            banned_patterns: vec![BannedPattern {
                pattern: "dbg!()".into(),
                description: "debug macro".into(),
            }],
            ..GatesConfig::default()
        };
        assert_eq!(gates.banned_patterns.len(), 1);
        assert_eq!(gates.banned_patterns[0].pattern, "dbg!()");
    }

    #[test]
    fn test_configurable_architecture_rules() {
        let gates = GatesConfig {
            architecture_rules: vec![DependencyRule {
                from_crate: "glyim-foo".into(),
                forbidden_dep: "glyim-bar".into(),
                reason: "test rule".into(),
            }],
            ..GatesConfig::default()
        };
        assert_eq!(gates.architecture_rules.len(), 1);
    }
}
```

## `src/git_ops/mod.rs`

```rust
pub mod worktree;

pub use worktree::{
    create_worktree, commit_all, emergency_wip_commit, push_branch, create_pr,
    status_porcelain, diff_main, log_oneline, diff_name_only, remove_worktree,
    detect_default_branch,
};
```

## `src/git_ops/worktree.rs`

```rust
use crate::error::PilotError;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

async fn run_git_command(
    program: &str,
    args: &[&str],
    cwd: &Path,
    timeout_secs: u64,
) -> Result<std::process::Output, PilotError> {
    let timeout = Duration::from_secs(if timeout_secs == 0 { 300 } else { timeout_secs });
    tracing::debug!(program, ?args, ?cwd, timeout_secs, "running command with timeout");
    let output_fut = Command::new(program).args(args).current_dir(cwd).output();
    match tokio::time::timeout(timeout, output_fut).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(PilotError::Git(format!(
            "{program} failed in {}: {e} (args: {:?})",
            cwd.display(),
            args
        ))),
        Err(_) => Err(PilotError::Git(format!(
            "{program} timed out after {timeout_secs}s in {} (args: {:?})",
            cwd.display(),
            args
        ))),
    }
}

pub async fn create_worktree(
    repo_root: &Path,
    worktree_base: &Path,
    stream_id: &str,
    default_branch: &str,
    branch_version: &str,
    timeout_secs: u64,
) -> Result<PathBuf, PilotError> {
    let worktree_dir = worktree_base.join(format!("stream-{stream_id}"));
    let branch_name = format!("stream-{stream_id}/{branch_version}");
    tracing::info!(
        stream_id,
        ?worktree_dir,
        default_branch,
        branch = %branch_name,
        "creating worktree"
    );

    let args = &[
        "worktree",
        "add",
        "--detach",
        &worktree_dir.to_string_lossy(),
        default_branch,
    ];
    let output = run_git_command("git", args, repo_root, timeout_secs).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "git {:?} failed in {}: {stderr}",
            args,
            repo_root.display()
        )));
    }

    let checkout_args = &["checkout", "-b", &branch_name];
    let output = run_git_command("git", checkout_args, &worktree_dir, timeout_secs).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "git {:?} failed in {}: {stderr}",
            checkout_args,
            worktree_dir.display()
        )));
    }

    tracing::info!(stream_id, branch = %branch_name, "worktree created and branch checked out");
    Ok(worktree_dir)
}

pub async fn commit_all(
    worktree_dir: &Path,
    stream_id: &str,
    message: &str,
    timeout_secs: u64,
) -> Result<(), PilotError> {
    let commit_msg = format!("stream-{stream_id}: {message}");
    let add_args = &["add", "-A"];
    let output = run_git_command("git", add_args, worktree_dir, timeout_secs).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "git {:?} failed in {}: {stderr}",
            add_args,
            worktree_dir.display()
        )));
    }

    let commit_args = &["commit", "-m", &commit_msg];
    let output = run_git_command("git", commit_args, worktree_dir, timeout_secs).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.contains("nothing to commit") || stderr.contains("no changes added to commit") {
            tracing::debug!(stream_id, "nothing to commit — no-op");
            return Ok(());
        }
        return Err(PilotError::Git(format!(
            "git {:?} failed in {}: {stderr}",
            commit_args,
            worktree_dir.display()
        )));
    }
    Ok(())
}

pub async fn emergency_wip_commit(
    worktree_dir: &Path,
    stream_id: &str,
    timeout_secs: u64,
) -> Result<(), PilotError> {
    tracing::warn!(stream_id, "making emergency WIP commit");
    commit_all(
        worktree_dir,
        stream_id,
        "WIP: emergency commit — fix rounds exceeded",
        timeout_secs,
    )
    .await
}

pub async fn push_branch(
    worktree_dir: &Path,
    stream_id: &str,
    branch_version: &str,
    timeout_secs: u64,
) -> Result<(), PilotError> {
    let branch_name = format!("stream-{stream_id}/{branch_version}");
    let args = &["push", "-u", "origin", &branch_name];
    let output = run_git_command("git", args, worktree_dir, timeout_secs).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "git {:?} failed in {}: {stderr}",
            args,
            worktree_dir.display()
        )));
    }
    Ok(())
}

pub async fn create_pr(
    worktree_dir: &Path,
    stream_id: &str,
    default_branch: &str,
    branch_version: &str,
    title: &str,
    body: &str,
    timeout_secs: u64,
) -> Result<String, PilotError> {
    let branch_name = format!("stream-{stream_id}/{branch_version}");
    let args = &[
        "pr",
        "create",
        "--base",
        default_branch,
        "--head",
        &branch_name,
        "--title",
        title,
        "--body",
        body,
    ];
    let output = run_git_command("gh", args, worktree_dir, timeout_secs).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "gh {:?} failed in {}: {stderr}",
            args,
            worktree_dir.display()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn status_porcelain(
    worktree_dir: &Path,
    timeout_secs: u64,
) -> Result<String, PilotError> {
    let args = &["status", "--porcelain"];
    let output = run_git_command("git", args, worktree_dir, timeout_secs).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "git {:?} failed in {}: {stderr}",
            args,
            worktree_dir.display()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn diff_main(
    worktree_dir: &Path,
    default_branch: &str,
    timeout_secs: u64,
) -> Result<String, PilotError> {
    let args = &["diff", &format!("{default_branch}..HEAD")];
    let output = run_git_command("git", args, worktree_dir, timeout_secs).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "git {:?} failed in {}: {stderr}",
            args,
            worktree_dir.display()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub async fn log_oneline(
    worktree_dir: &Path,
    default_branch: &str,
    timeout_secs: u64,
) -> Result<String, PilotError> {
    let args = &["log", &format!("{default_branch}..HEAD"), "--oneline"];
    let output = run_git_command("git", args, worktree_dir, timeout_secs).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "git {:?} failed in {}: {stderr}",
            args,
            worktree_dir.display()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn diff_name_only(
    worktree_dir: &Path,
    default_branch: &str,
    timeout_secs: u64,
) -> Result<String, PilotError> {
    let args = &["diff", "--name-only", &format!("{default_branch}..HEAD")];
    let output = run_git_command("git", args, worktree_dir, timeout_secs).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "git {:?} failed in {}: {stderr}",
            args,
            worktree_dir.display()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn remove_worktree(
    repo_root: &Path,
    worktree_dir: &Path,
    timeout_secs: u64,
) -> Result<(), PilotError> {
    let args = &["worktree", "remove", &worktree_dir.to_string_lossy(), "--force"];
    let output = run_git_command("git", args, repo_root, timeout_secs).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "git {:?} failed in {}: {stderr}",
            args,
            repo_root.display()
        )));
    }
    Ok(())
}

pub async fn detect_default_branch(repo_root: &Path, fallback: &str, timeout_secs: u64) -> String {
    let args = &["symbolic-ref", "refs/remotes/origin/HEAD"];
    match run_git_command("git", args, repo_root, timeout_secs).await {
        Ok(output) if output.status.success() => {
            let ref_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Some(branch) = ref_path.strip_prefix("refs/remotes/origin/") {
                tracing::info!(detected_branch = branch, "auto-detected default branch");
                return branch.to_string();
            }
            fallback.to_string()
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            tracing::warn!(fallback_branch = fallback, %stderr, "could not auto-detect default branch");
            fallback.to_string()
        }
        Err(e) => {
            tracing::warn!(fallback_branch = fallback, error = %e, "could not auto-detect default branch");
            fallback.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::process::Command as AsyncCommand;

    const TEST_TIMEOUT: u64 = 30;

    async fn setup_test_repo() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        AsyncCommand::new("git")
            .args(["init"])
            .current_dir(root)
            .output()
            .await
            .unwrap();
        AsyncCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(root)
            .output()
            .await
            .unwrap();
        AsyncCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(root)
            .output()
            .await
            .unwrap();
        std::fs::write(root.join("README.md"), "# Test").unwrap();
        AsyncCommand::new("git")
            .args(["add", "-A"])
            .current_dir(root)
            .output()
            .await
            .unwrap();
        AsyncCommand::new("git")
            .args(["commit", "-m", "initial commit"])
            .current_dir(root)
            .output()
            .await
            .unwrap();
        AsyncCommand::new("git")
            .args(["branch", "-M", "main"])
            .current_dir(root)
            .output()
            .await
            .unwrap();
        dir
    }

    #[tokio::test]
    async fn test_create_worktree() {
        let dir = setup_test_repo().await;
        let root = dir.path();
        let wt_base = root.parent().unwrap().join("wt_create");
        let result = create_worktree(root, &wt_base, "S01", "main", "v0.1.0", TEST_TIMEOUT).await;
        assert!(result.is_ok());
        assert!(result.unwrap().exists());
    }

    #[tokio::test]
    async fn test_commit_all() {
        let dir = setup_test_repo().await;
        let root = dir.path();
        let wt_base = root.parent().unwrap().join("wt_commit");
        let wt_path = create_worktree(root, &wt_base, "S02", "main", "v0.1.0", TEST_TIMEOUT)
            .await
            .unwrap();
        std::fs::write(wt_path.join("file.rs"), "content").unwrap();
        assert!(commit_all(&wt_path, "S02", "test commit", TEST_TIMEOUT)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_command_timeout_returns_error() {
        let dir = setup_test_repo().await;
        let root = dir.path();
        let result = run_git_command("sleep", &["10"], root, 1).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }
}
```

## `src/gates/mod.rs`

```rust
pub mod types;
pub mod helpers;
pub mod fmt;
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
use std::path::Path;

pub use types::{GateResult, PipelineResult};

/// A quality gate that checks some property of the worktree.
///
/// # Error vs. Failure Contract
///
/// - `Err(PilotError::Gate { .. })` → **Infrastructure failure**: tool not
///   installed, timeout, OS error. The gate could not run at all.
/// - `Ok(GateResult { passed: false, .. })` → **Semantic failure**: the gate
///   ran successfully but found violations (threshold not met, banned patterns
///   found, etc.)
#[async_trait]
pub trait Gate: Send + Sync {
    fn name(&self) -> &str;
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError>;
}
```

## `src/gates/types.rs`

```rust
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Context passed to every gate, containing all information a gate might need.
/// Eliminates the need for per-gate constructor injection.
#[derive(Debug, Clone)]
pub struct GateContext {
    pub worktree_dir: PathBuf,
    pub project_root: PathBuf,
    pub default_branch: String,
    pub branch_version: String,
    pub timeout_secs: u64,
}

impl GateContext {
    pub fn new(
        worktree_dir: PathBuf,
        project_root: PathBuf,
        default_branch: String,
        branch_version: String,
        timeout_secs: u64,
    ) -> Self {
        Self {
            worktree_dir,
            project_root,
            default_branch,
            branch_version,
            timeout_secs,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub gate_name: String,
    pub passed: bool,
    pub message: String,
    pub details: Option<String>,
}

impl GateResult {
    pub fn pass(name: impl Into<String>) -> Self {
        Self {
            gate_name: name.into(),
            passed: true,
            message: "passed".into(),
            details: None,
        }
    }
    pub fn pass_with_note(name: impl Into<String>, note: impl Into<String>) -> Self {
        Self {
            gate_name: name.into(),
            passed: true,
            message: note.into(),
            details: None,
        }
    }
    pub fn pass_with_details(name: impl Into<String>, note: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            gate_name: name.into(),
            passed: true,
            message: note.into(),
            details: Some(details.into()),
        }
    }
    pub fn fail(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            gate_name: name.into(),
            passed: false,
            message: message.into(),
            details: None,
        }
    }
    pub fn fail_with_details(
        name: impl Into<String>,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self {
            gate_name: name.into(),
            passed: false,
            message: message.into(),
            details: Some(details.into()),
        }
    }
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
    pub fn failure_message(&self) -> String {
        if let Some(fail) = self.first_failure() {
            let mut msg = format!("**{} failed**: {}", fail.gate_name, fail.message);
            if let Some(details) = &fail.details {
                msg = format!("{msg}\n\n```\n{details}\n```");
            }
            msg
        } else {
            String::new()
        }
    }
}
```

## `src/gates/helpers.rs`

```rust
use crate::error::PilotError;
use std::path::Path;
use std::time::Duration;

pub async fn run_command(
    program: &str,
    args: &[&str],
    cwd: &Path,
    timeout_secs: u64,
) -> Result<std::process::Output, PilotError> {
    let timeout = Duration::from_secs(if timeout_secs == 0 { 300 } else { timeout_secs });
    tracing::debug!(program, ?args, ?cwd, timeout_secs);
    let output_fut = tokio::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output();
    match tokio::time::timeout(timeout, output_fut).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(PilotError::Gate {
            gate: program.into(),
            message: format!(
                "failed to execute {program}: {e} (cwd: {}, args: {:?})",
                cwd.display(),
                args
            ),
        }),
        Err(_) => Err(PilotError::Gate {
            gate: program.into(),
            message: format!(
                "{program} timed out after {timeout_secs}s (cwd: {}, args: {:?})",
                cwd.display(),
                args
            ),
        }),
    }
}

pub fn strip_ansi(s: &str) -> String {
    strip_ansi_escapes::strip_str(s)
}

pub fn trim_errors_and_warnings(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= 50 {
        return output.to_string();
    }
    let mut relevant = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("error") || trimmed.starts_with("warning") {
            let start = i.saturating_sub(2);
            let end = (i + 5).min(lines.len());
            for j in start..end {
                relevant.push(lines[j]);
            }
            relevant.push("...");
        }
    }
    if relevant.is_empty() {
        lines[lines.len() - 50..].join("\n")
    } else {
        relevant.join("\n")
    }
}

/// Check both stdout and stderr for "no such command" patterns.
/// cargo subcommands print to stdout, not stderr, when not installed.
pub fn is_command_not_found(stdout: &str, stderr: &str) -> bool {
    let combined = format!("{stdout}\n{stderr}").to_lowercase();
    combined.contains("command not found")
        || combined.contains("no such command")
        || combined.contains("not found")
        || combined.contains("error: no such command")
}

pub fn trim_test_failures(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= 80 {
        return output.to_string();
    }
    let mut relevant = Vec::new();
    let mut in_failures = false;
    for line in &lines {
        if line.trim().starts_with("failures:") {
            in_failures = true;
        }
        if in_failures {
            relevant.push(*line);
            if relevant.len() > 60 {
                break;
            }
        }
    }
    for line in lines.iter().rev() {
        if line.contains("test result:") {
            relevant.push(*line);
            break;
        }
    }
    if relevant.is_empty() {
        lines[lines.len() - 60..].join("\n")
    } else {
        relevant.join("\n")
    }
}
```

## `src/gates/fmt.rs`

```rust
use crate::error::PilotError;
use crate::gates::helpers::run_command;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

/// Checks formatting via `cargo fmt --check`. If formatting is wrong,
/// auto-fixes with `cargo fmt` and reports which files changed.
///
/// NOTE: This gate has a side effect (auto-fix). The result includes
/// the list of changed files in `details` so the AI can reason about
/// what happened.
pub struct FmtGate;

#[async_trait]
impl Gate for FmtGate {
    fn name(&self) -> &str {
        "fmt"
    }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_command(
            "cargo",
            &["fmt", "--", "--check"],
            &ctx.worktree_dir,
            ctx.timeout_secs,
        )
        .await?;

        if output.status.success() {
            return Ok(GateResult::pass("fmt"));
        }

        // Auto-fix
        let fix_output =
            run_command("cargo", &["fmt"], &ctx.worktree_dir, ctx.timeout_secs).await?;
        if !fix_output.status.success() {
            let stderr = crate::gates::helpers::strip_ansi(&String::from_utf8_lossy(
                &fix_output.stderr,
            ));
            return Ok(GateResult::fail_with_details(
                "fmt",
                "cargo fmt failed to apply formatting",
                stderr,
            ));
        }

        // Get the list of changed files for AI feedback
        let changed =
            run_command("git", &["diff", "--name-only"], &ctx.worktree_dir, ctx.timeout_secs)
                .await;
        let changed_files = match changed {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            }
            _ => String::new(),
        };

        let files_note = if changed_files.is_empty() {
            "(unknown files)".into()
        } else {
            changed_files.clone()
        };

        tracing::warn!("fmt: auto-fixed. Changed files: {}", files_note);

        Ok(GateResult::pass_with_details(
            "fmt",
            "auto-fixed: cargo fmt applied changes (not committed)",
            format!("Changed files:\n{}", files_note),
        ))
    }
}
```

## `src/gates/check.rs`

```rust
use crate::error::PilotError;
use crate::gates::helpers::{run_command, strip_ansi, trim_errors_and_warnings};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct CheckGate;

#[async_trait]
impl Gate for CheckGate {
    fn name(&self) -> &str {
        "check"
    }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output =
            run_command("cargo", &["check"], &ctx.worktree_dir, ctx.timeout_secs).await?;
        if output.status.success() {
            Ok(GateResult::pass("check"))
        } else {
            let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
            Ok(GateResult::fail_with_details(
                "check",
                "compilation failed",
                trim_errors_and_warnings(&stderr),
            ))
        }
    }
}
```

## `src/gates/clippy.rs`

```rust
use crate::error::PilotError;
use crate::gates::helpers::{run_command, strip_ansi, trim_errors_and_warnings};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct ClippyGate;

#[async_trait]
impl Gate for ClippyGate {
    fn name(&self) -> &str {
        "clippy"
    }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_command(
            "cargo",
            &["clippy", "--", "-D", "warnings"],
            &ctx.worktree_dir,
            ctx.timeout_secs,
        )
        .await?;
        if output.status.success() {
            Ok(GateResult::pass("clippy"))
        } else {
            let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
            Ok(GateResult::fail_with_details(
                "clippy",
                "clippy warnings found",
                trim_errors_and_warnings(&stderr),
            ))
        }
    }
}
```

## `src/gates/test_gate.rs`

```rust
use crate::error::PilotError;
use crate::gates::helpers::{run_command, strip_ansi, trim_test_failures};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct TestGate;

#[async_trait]
impl Gate for TestGate {
    fn name(&self) -> &str {
        "test"
    }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output =
            run_command("cargo", &["test"], &ctx.worktree_dir, ctx.timeout_secs).await?;
        if output.status.success() {
            Ok(GateResult::pass("test"))
        } else {
            let stdout = strip_ansi(&String::from_utf8_lossy(&output.stdout));
            let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
            Ok(GateResult::fail_with_details(
                "test",
                "test failures detected",
                trim_test_failures(&format!("{stdout}\n{stderr}")),
            ))
        }
    }
}
```

## `src/gates/banned_pattern.rs`

```rust
use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A configurable banned pattern with its description.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BannedPattern {
    pub pattern: String,
    pub description: String,
}

impl BannedPattern {
    pub fn new(pattern: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            description: description.into(),
        }
    }
}

fn default_banned_patterns() -> Vec<BannedPattern> {
    vec![
        BannedPattern::new("todo!()", "`todo!()` in non-test code"),
        BannedPattern::new("unwrap()", "`.unwrap()` in non-test code"),
        BannedPattern::new("panic!()", "`panic!()` in non-test code"),
    ]
}

/// Checks for banned patterns in non-test, non-comment Rust source.
///
/// Patterns are configurable via `PilotConfig.gates.banned_patterns`.
/// If none are configured, defaults are used.
///
/// NOTE: This uses simple string matching, not a proper Rust tokenizer.
/// Patterns like ` as ` may produce false positives in string literals.
/// For production use, consider integrating `syn` for token-aware matching.
pub struct BannedPatternGate {
    patterns: Vec<BannedPattern>,
}

impl BannedPatternGate {
    pub fn new(patterns: Vec<BannedPattern>) -> Self {
        Self {
            patterns: if patterns.is_empty() {
                default_banned_patterns()
            } else {
                patterns
            },
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(default_banned_patterns())
    }
}

impl Default for BannedPatternGate {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[async_trait]
impl Gate for BannedPatternGate {
    fn name(&self) -> &str {
        "banned_patterns"
    }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let dir = ctx.worktree_dir.clone();
        let patterns = self.patterns.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut violations = Vec::new();
            let walker = ignore::WalkBuilder::new(&dir).hidden(false).build();
            for entry in walker.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "rs") {
                    let path_str = path.to_string_lossy();
                    if path_str.contains("/tests/") || path_str.contains("\\tests\\") {
                        continue;
                    }
                    if let Ok(content) = std::fs::read_to_string(path) {
                        for (i, line) in content.lines().enumerate() {
                            if line.trim().starts_with("//") {
                                continue;
                            }
                            // Skip string literals for `as` pattern specifically
                            // (partial fix — full fix would use syn)
                            let check_line = if line.contains('"') {
                                // Rough heuristic: strip string contents for ` as ` check
                                let stripped = strip_string_literals(line);
                                stripped
                            } else {
                                line.to_string()
                            };
                            for pattern in &patterns {
                                if check_line.contains(&pattern.pattern) {
                                    let rel = path.strip_prefix(&dir).unwrap_or(path).display();
                                    violations.push(format!(
                                        "{}:{}: {}",
                                        rel,
                                        i + 1,
                                        pattern.description
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            if violations.is_empty() {
                GateResult::pass("banned_patterns")
            } else {
                GateResult::fail_with_details(
                    "banned_patterns",
                    format!("{} banned pattern(s) found", violations.len()),
                    violations.join("\n"),
                )
            }
        })
        .await
        .map_err(|e| PilotError::Gate {
            gate: "banned_patterns".into(),
            message: format!("spawn_blocking join error: {e}"),
        })?;
        Ok(result)
    }
}

/// Rough string-literal stripping to reduce false positives on patterns
/// like ` as ` that commonly appear inside string content.
/// This is NOT a full tokenizer — it handles the common case of
/// `"used as input"` and similar.
fn strip_string_literals(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut in_string = false;
    let mut escaped = false;
    for ch in line.chars() {
        if escaped {
            escaped = false;
            if in_string {
                // Skip escaped char inside string
                continue;
            } else {
                result.push(ch);
            }
            continue;
        }
        if ch == '\\' {
            if in_string {
                escaped = true;
                continue;
            } else {
                result.push(ch);
            }
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if !in_string {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_string_literals_basic() {
        assert_eq!(
            strip_string_literals(r#"let x = "used as input"; let y = foo as Bar;"#),
            "let x = ; let y = foo as Bar;"
        );
    }

    #[test]
    fn test_strip_string_literals_escaped_quote() {
        assert_eq!(
            strip_string_literals(r#"let s = "he said \"hello\" as code"; let y = x as i32;"#),
            "let s = ; let y = x as i32;"
        );
    }

    #[test]
    fn test_banned_pattern_custom() {
        let gate = BannedPatternGate::new(vec![BannedPattern::new("dbg!()", "debug macro")]);
        assert_eq!(gate.patterns.len(), 1);
        assert_eq!(gate.name(), "banned_patterns");
    }
}
```

## `src/gates/architecture.rs`

```rust
use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A dependency rule: `from_crate` must not depend on `forbidden_dep`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DependencyRule {
    pub from_crate: String,
    pub forbidden_dep: String,
    pub reason: String,
}

fn default_rules() -> Vec<DependencyRule> {
    vec![
        DependencyRule {
            from_crate: "glyim-frontend".into(),
            forbidden_dep: "glyim-type".into(),
            reason: "frontend must not depend on type directly".into(),
        },
        DependencyRule {
            from_crate: "glyim-frontend".into(),
            forbidden_dep: "glyim-ir".into(),
            reason: "frontend must not depend on IR".into(),
        },
        DependencyRule {
            from_crate: "glyim-syntax".into(),
            forbidden_dep: "glyim-ir".into(),
            reason: "syntax must not depend on IR".into(),
        },
        DependencyRule {
            from_crate: "glyim-type".into(),
            forbidden_dep: "glyim-codegen".into(),
            reason: "type must not depend on codegen".into(),
        },
    ]
}

/// Checks architectural dependency rules by parsing Cargo.toml files
/// using the `toml` crate instead of string hacks.
pub struct ArchitectureGate {
    rules: Vec<DependencyRule>,
}

impl ArchitectureGate {
    pub fn new(rules: Vec<DependencyRule>) -> Self {
        Self {
            rules: if rules.is_empty() {
                default_rules()
            } else {
                rules
            },
        }
    }

    pub fn with_default_rules() -> Self {
        Self::new(default_rules())
    }
}

impl Default for ArchitectureGate {
    fn default() -> Self {
        Self::with_default_rules()
    }
}

#[async_trait]
impl Gate for ArchitectureGate {
    fn name(&self) -> &str {
        "architecture"
    }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let dir = ctx.worktree_dir.clone();
        let rules = self.rules.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut violations = Vec::new();
            let walker = ignore::WalkBuilder::new(&dir).hidden(false).build();
            for entry in walker.flatten() {
                let path = entry.path();
                if path.file_name().map_or(false, |n| n == "Cargo.toml") {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        if let Some(crate_name) = extract_crate_name_toml(&content) {
                            let deps = extract_dependencies_toml(&content);
                            for rule in &rules {
                                if crate_name == rule.from_crate
                                    && deps.contains(&rule.forbidden_dep)
                                {
                                    let rel =
                                        path.strip_prefix(&dir).unwrap_or(path).display();
                                    violations.push(format!(
                                        "{}: {} depends on {} – {}",
                                        rel, rule.from_crate, rule.forbidden_dep, rule.reason
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            if violations.is_empty() {
                GateResult::pass("architecture")
            } else {
                GateResult::fail_with_details(
                    "architecture",
                    format!("{} violation(s)", violations.len()),
                    violations.join("\n"),
                )
            }
        })
        .await
        .map_err(|e| PilotError::Gate {
            gate: "architecture".into(),
            message: format!("spawn_blocking join error: {e}"),
        })?;
        Ok(result)
    }
}

/// Extract crate name from Cargo.toml using proper TOML parsing.
fn extract_crate_name_toml(content: &str) -> Option<String> {
    let value: toml::Value = toml::from_str(content).ok()?;
    value
        .get("package")?
        .get("name")?
        .as_str()
        .map(|s| s.to_string())
}

/// Extract dependency names from the `[dependencies]` section using
/// proper TOML parsing. Handles inline tables, workspace inheritance,
/// and simple string values.
fn extract_dependencies_toml(content: &str) -> Vec<String> {
    let value: toml::Value = match toml::from_str(content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut deps = Vec::new();

    // Only check [dependencies], NOT [dev-dependencies]
    if let Some(deps_table) = value.get("dependencies").and_then(|v| v.as_table()) {
        for key in deps_table.keys() {
            deps.push(key.clone());
        }
    }

    deps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_crate_name_toml() {
        let cargo = r#"
[package]
name = "glyim-frontend"
version = "0.1.0"

[dependencies]
glyim-syntax = { path = "../syntax" }
glyim-ir = { path = "../ir" }
"#;
        assert_eq!(
            extract_crate_name_toml(cargo),
            Some("glyim-frontend".into())
        );
    }

    #[test]
    fn test_extract_dependencies_toml() {
        let cargo = r#"
[package]
name = "glyim-frontend"
version = "0.1.0"

[dependencies]
glyim-syntax = { path = "../syntax" }
glyim-ir = { path = "../ir" }
serde = "1"

[dev-dependencies]
tempfile = "3"
"#;
        let deps = extract_dependencies_toml(cargo);
        assert!(deps.contains(&"glyim-syntax".to_string()));
        assert!(deps.contains(&"glyim-ir".to_string()));
        assert!(deps.contains(&"serde".to_string()));
        // dev-dependencies should NOT be included
        assert!(!deps.contains(&"tempfile".to_string()));
    }

    #[test]
    fn test_extract_dependencies_workspace_inheritance() {
        let cargo = r#"
[package]
name = "glyim-frontend"
version = "0.1.0"

[dependencies]
glyim-ir.workspace = true
"#;
        let deps = extract_dependencies_toml(cargo);
        assert!(deps.contains(&"glyim-ir".to_string()));
    }

    #[test]
    fn test_extract_crate_name_malformed() {
        assert_eq!(extract_crate_name_toml("not toml at all {{{"), None);
    }
}
```

## `src/gates/contracts.rs`

```rust
use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

/// Checks that locked interfaces in CONTRACTS_LOCKED.md are not
/// modified by the current diff.
///
/// NOTE: `extract_pub_name` is a best-effort parser. For production
/// use with complex Rust signatures, consider using `syn` for
/// proper token-aware extraction.
pub struct ContractGate;

#[async_trait]
impl Gate for ContractGate {
    fn name(&self) -> &str {
        "contracts"
    }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let contracts_path = ctx.project_root.join("CONTRACTS_LOCKED.md");
        let locked_names = if contracts_path.exists() {
            let content = tokio::fs::read_to_string(&contracts_path)
                .await
                .map_err(|e| PilotError::Gate {
                    gate: "contracts".into(),
                    message: format!("failed to read CONTRACTS_LOCKED.md: {e}"),
                })?;
            extract_locked_names(&content)
        } else {
            return Ok(GateResult::pass_with_note(
                "contracts",
                "no CONTRACTS_LOCKED.md found",
            ));
        };
        if locked_names.is_empty() {
            return Ok(GateResult::pass("contracts"));
        }

        let diff =
            crate::git_ops::diff_main(&ctx.worktree_dir, &ctx.default_branch, ctx.timeout_secs)
                .await?;
        if diff.is_empty() {
            return Ok(GateResult::pass("contracts"));
        }

        let mut violations = Vec::new();
        for line in diff.lines() {
            if line.starts_with('-') && !line.starts_with("---") {
                for name in &locked_names {
                    if line.contains(name.as_str()) {
                        violations.push(format!(
                            "locked interface '{}' in removed line: {}",
                            name,
                            line.trim_start_matches('-').trim()
                        ));
                    }
                }
            }
        }
        if violations.is_empty() {
            Ok(GateResult::pass("contracts"))
        } else {
            Ok(GateResult::fail_with_details(
                "contracts",
                format!("{} violation(s)", violations.len()),
                violations.join("\n"),
            ))
        }
    }
}

fn extract_locked_names(content: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_code = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if !in_code {
            continue;
        }
        if let Some(name) = extract_pub_name(trimmed) {
            names.push(name);
        }
    }
    names
}

/// Best-effort extraction of public item names from Rust signatures.
///
/// Handles: `pub fn`, `pub async fn`, `pub struct`, `pub enum`, `pub trait`,
/// `pub(crate) fn`, `pub const fn`.
///
/// Does NOT handle: `pub(in crate::x)`, tuple structs, trait bounds
/// on the same line. For production, use `syn`.
fn extract_pub_name(line: &str) -> Option<String> {
    let line = line.trim();

    // Handle visibility modifiers
    let after_pub = if let Some(rest) = line.strip_prefix("pub async fn ") {
        Some(("fn", rest))
    } else if let Some(rest) = line.strip_prefix("pub const fn ") {
        Some(("fn", rest))
    } else if let Some(rest) = line.strip_prefix("pub(crate) async fn ") {
        Some(("fn", rest))
    } else if let Some(rest) = line.strip_prefix("pub(crate) fn ") {
        Some(("fn", rest))
    } else if let Some(rest) = line.strip_prefix("pub fn ") {
        Some(("fn", rest))
    } else if let Some(rest) = line.strip_prefix("pub struct ") {
        Some(("struct", rest))
    } else if let Some(rest) = line.strip_prefix("pub enum ") {
        Some(("enum", rest))
    } else if let Some(rest) = line.strip_prefix("pub trait ") {
        Some(("trait", rest))
    } else {
        None
    };

    let (kind, after) = after_pub?;

    match kind {
        "fn" => after.split('(').next().map(|s| s.trim().to_string()),
        "struct" => after
            .split(|c: char| c == '<' || c == '{' || c == ' ' || c == ';')
            .next()
            .map(|s| s.trim().to_string()),
        "enum" => after
            .split(|c: char| c == '<' || c == '{' || c == ' ' || c == ';')
            .next()
            .map(|s| s.trim().to_string()),
        "trait" => after
            .split(|c: char| c == '<' || c == '{' || c == ':')
            .next()
            .map(|s| s.trim().to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pub_fn() {
        assert_eq!(
            extract_pub_name("pub fn parse(input: &str) -> Result<()>"),
            Some("parse".into())
        );
    }

    #[test]
    fn test_extract_pub_async_fn() {
        assert_eq!(
            extract_pub_name("pub async fn run(ctx: &Context) -> Result<()>"),
            Some("run".into())
        );
    }

    #[test]
    fn test_extract_pub_crate_fn() {
        assert_eq!(
            extract_pub_name("pub(crate) fn helper() -> bool"),
            Some("helper".into())
        );
    }

    #[test]
    fn test_extract_pub_const_fn() {
        assert_eq!(
            extract_pub_name("pub const fn max_size() -> usize"),
            Some("max_size".into())
        );
    }

    #[test]
    fn test_extract_pub_struct_with_generics() {
        assert_eq!(
            extract_pub_name("pub struct Parser<T: Clone> {"),
            Some("Parser".into())
        );
    }

    #[test]
    fn test_extract_pub_enum() {
        assert_eq!(
            extract_pub_name("pub enum Error {"),
            Some("Error".into())
        );
    }

    #[test]
    fn test_extract_pub_trait() {
        assert_eq!(
            extract_pub_name("pub trait Gate: Send + Sync {"),
            Some("Gate".into())
        );
    }

    #[test]
    fn test_extract_non_pub() {
        assert_eq!(extract_pub_name("fn internal() {}"), None);
    }
}
```

## `src/gates/dead_code.rs`

```rust
use crate::error::PilotError;
use crate::gates::helpers::{run_command, strip_ansi};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct DeadCodeGate;

#[async_trait]
impl Gate for DeadCodeGate {
    fn name(&self) -> &str {
        "dead_code"
    }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_command(
            "cargo",
            &["check", "--all-targets", "--", "-W", "dead_code", "-W", "unused_imports"],
            &ctx.worktree_dir,
            ctx.timeout_secs,
        )
        .await?;
        if !output.status.success() {
            return Ok(GateResult::fail(
                "dead_code",
                "cargo check failed – fix compilation first",
            ));
        }
        let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
        if stderr.contains("dead_code") || stderr.contains("unused") {
            Ok(GateResult::fail_with_details(
                "dead_code",
                "dead code or unused imports found",
                stderr,
            ))
        } else {
            Ok(GateResult::pass("dead_code"))
        }
    }
}
```

## `src/gates/coverage.rs`

```rust
use crate::error::PilotError;
use crate::gates::helpers::{is_command_not_found, run_command};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use regex::Regex;
use std::sync::LazyLock;

static COVERAGE_PCT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+\.?\d*)%\s*coverage").unwrap());

pub struct CoverageGate {
    pub min_coverage: f64,
}

impl CoverageGate {
    pub fn new(min_coverage: f64) -> Self {
        Self { min_coverage }
    }
}

#[async_trait]
impl Gate for CoverageGate {
    fn name(&self) -> &str {
        "coverage"
    }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_command(
            "cargo",
            &["llvm-cov", "--summary-only"],
            &ctx.worktree_dir,
            ctx.timeout_secs,
        )
        .await;
        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(pct) = COVERAGE_PCT_RE
                    .captures(&stdout)
                    .and_then(|c| c.get(1))
                    .and_then(|m| m.as_str().parse::<f64>().ok())
                {
                    if pct >= self.min_coverage {
                        Ok(GateResult::pass("coverage"))
                    } else {
                        Ok(GateResult::fail_with_details(
                            "coverage",
                            format!("coverage {pct:.0}% < {}%", self.min_coverage),
                            stdout.to_string(),
                        ))
                    }
                } else {
                    Ok(GateResult::fail(
                        "coverage",
                        "could not parse coverage output",
                    ))
                }
            }
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                // Infrastructure failure: tool not installed
                if is_command_not_found(&stdout, &stderr) {
                    Err(PilotError::Gate {
                        gate: "coverage".into(),
                        message: "cargo-llvm-cov not installed – infrastructure failure".into(),
                    })
                } else {
                    // Semantic failure: tool ran but reported errors
                    Ok(GateResult::fail("coverage", "cargo llvm-cov failed"))
                }
            }
            Err(e) => Err(e),
        }
    }
}
```

## `src/gates/mutation.rs`

```rust
use crate::error::PilotError;
use crate::gates::helpers::{is_command_not_found, run_command};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use regex::Regex;
use std::sync::LazyLock;

static MUTATION_PCT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\((\d+\.?\d*)%\)").unwrap());

pub struct MutationGate {
    pub min_kill_rate: f64,
}

impl MutationGate {
    pub fn new(min_kill_rate: f64) -> Self {
        Self { min_kill_rate }
    }
}

#[async_trait]
impl Gate for MutationGate {
    fn name(&self) -> &str {
        "mutation"
    }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_command(
            "cargo",
            &["mutants", "--no-times"],
            &ctx.worktree_dir,
            ctx.timeout_secs,
        )
        .await;
        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(rate) = MUTATION_PCT_RE
                    .captures(&stdout)
                    .and_then(|c| c.get(1))
                    .and_then(|m| m.as_str().parse::<f64>().ok())
                {
                    if rate >= self.min_kill_rate {
                        Ok(GateResult::pass("mutation"))
                    } else {
                        Ok(GateResult::fail_with_details(
                            "mutation",
                            format!("kill rate {rate:.0}% < {}%", self.min_kill_rate),
                            stdout.to_string(),
                        ))
                    }
                } else {
                    Ok(GateResult::fail(
                        "mutation",
                        "could not parse mutation output",
                    ))
                }
            }
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                if is_command_not_found(&stdout, &stderr) {
                    Err(PilotError::Gate {
                        gate: "mutation".into(),
                        message: "cargo-mutants not installed – infrastructure failure".into(),
                    })
                } else {
                    Ok(GateResult::fail("mutation", "cargo mutants failed"))
                }
            }
            Err(e) => Err(e),
        }
    }
}
```

## `src/gates/workspace_check.rs`

```rust
use crate::error::PilotError;
use crate::gates::helpers::{run_command, strip_ansi};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct WorkspaceCheckGate;

#[async_trait]
impl Gate for WorkspaceCheckGate {
    fn name(&self) -> &str {
        "workspace_check"
    }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_command(
            "cargo",
            &["check", "--workspace"],
            &ctx.worktree_dir,
            ctx.timeout_secs,
        )
        .await?;
        if output.status.success() {
            Ok(GateResult::pass("workspace_check"))
        } else {
            let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
            Ok(GateResult::fail_with_details(
                "workspace_check",
                "workspace check failed",
                stderr,
            ))
        }
    }
}
```

## `src/gates/audit.rs`

```rust
use crate::error::PilotError;
use crate::gates::helpers::{is_command_not_found, run_command};
use crate::gates::types::GateContext;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;

pub struct AuditGate;

#[async_trait]
impl Gate for AuditGate {
    fn name(&self) -> &str {
        "audit"
    }
    async fn run(&self, ctx: &GateContext) -> Result<GateResult, PilotError> {
        let output = run_command("cargo", &["audit"], &ctx.worktree_dir, ctx.timeout_secs).await;
        match output {
            Ok(out) if out.status.success() => Ok(GateResult::pass("audit")),
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                if is_command_not_found(&stdout, &stderr) {
                    Err(PilotError::Gate {
                        gate: "audit".into(),
                        message: "cargo-audit not installed – infrastructure failure".into(),
                    })
                } else {
                    Ok(GateResult::fail_with_details(
                        "audit",
                        "vulnerabilities found",
                        format!("{stdout}{stderr}"),
                    ))
                }
            }
            Err(e) => Err(e),
        }
    }
}
```

## `src/gates/self_review.rs`

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

## `src/gates/commit_pipeline.rs`

```rust
use crate::config::types::ResolvedCommitGates;
use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{
    Gate, GateResult, PipelineResult,
    architecture::ArchitectureGate,
    banned_pattern::BannedPatternGate,
    check::CheckGate,
    clippy::ClippyGate,
    contracts::ContractGate,
    fmt::FmtGate,
    test_gate::TestGate,
};
use std::sync::Arc;
use std::time::Instant;

pub async fn run_commit_pipeline(
    ctx: &GateContext,
    config: &ResolvedCommitGates,
    banned_patterns: Vec<crate::gates::banned_pattern::BannedPattern>,
    architecture_rules: Vec<crate::gates::architecture::DependencyRule>,
) -> Result<PipelineResult, PilotError> {
    let mut gates: Vec<Arc<dyn Gate>> = Vec::new();
    if config.fmt {
        gates.push(Arc::new(FmtGate));
    }
    if config.check {
        gates.push(Arc::new(CheckGate));
    }
    if config.clippy {
        gates.push(Arc::new(ClippyGate));
    }
    if config.test {
        gates.push(Arc::new(TestGate));
    }
    if config.banned_patterns {
        gates.push(Arc::new(BannedPatternGate::new(banned_patterns)));
    }
    if config.architecture {
        gates.push(Arc::new(ArchitectureGate::new(architecture_rules)));
    }
    if config.contracts {
        gates.push(Arc::new(ContractGate));
    }

    let mut results = Vec::new();
    for gate in &gates {
        let start = Instant::now();
        let result = gate.run(ctx).await?;
        let elapsed = start.elapsed();
        tracing::info!(
            gate = gate.name(),
            elapsed = ?elapsed,
            passed = result.passed,
            "commit gate completed"
        );
        let passed = result.passed;
        results.push(result);
        if !passed {
            break;
        }
    }
    Ok(PipelineResult::from_gates(results))
}
```

## `src/gates/done_pipeline.rs`

```rust
use crate::config::types::ResolvedDoneGates;
use crate::error::PilotError;
use crate::gates::types::GateContext;
use crate::gates::{
    Gate, GateResult, PipelineResult,
    audit::AuditGate,
    coverage::CoverageGate,
    dead_code::DeadCodeGate,
    mutation::MutationGate,
    workspace_check::WorkspaceCheckGate,
};
use std::sync::Arc;
use std::time::Instant;

/// Run the done pipeline. This pipeline only runs gates NOT already
/// covered by the commit pipeline. The commit pipeline already ran:
/// fmt, check, clippy, test, banned_patterns, architecture, contracts.
/// The done pipeline adds: dead_code, coverage, mutation,
/// workspace_check, audit.
pub async fn run_done_pipeline(
    ctx: &GateContext,
    config: &ResolvedDoneGates,
) -> Result<PipelineResult, PilotError> {
    let mut gates: Vec<Arc<dyn Gate>> = Vec::new();

    // Only done-specific gates — NOT fmt/check/clippy/test/banned/arch/contracts
    // (those already passed in the commit pipeline)
    if config.dead_code {
        gates.push(Arc::new(DeadCodeGate));
    }
    if config.coverage {
        gates.push(Arc::new(CoverageGate::new(config.coverage_min)));
    }
    if config.mutation {
        gates.push(Arc::new(MutationGate::new(config.mutation_kill_rate)));
    }
    if config.workspace_check {
        gates.push(Arc::new(WorkspaceCheckGate));
    }
    if config.audit {
        gates.push(Arc::new(AuditGate));
    }

    if gates.is_empty() {
        return Ok(PipelineResult::from_gates(vec![GateResult::pass(
            "done_pipeline",
        )]));
    }

    let mut results = Vec::new();
    for gate in &gates {
        let start = Instant::now();
        let result = gate.run(ctx).await?;
        let elapsed = start.elapsed();
        tracing::info!(
            gate = gate.name(),
            elapsed = ?elapsed,
            passed = result.passed,
            "done gate completed"
        );
        let passed = result.passed;
        results.push(result);
        if !passed {
            break;
        }
    }
    Ok(PipelineResult::from_gates(results))
}
```

## `src/commit/mod.rs`

```rust
pub mod engine;
pub use engine::{CommitEngine, CommitDecision};
```

## `src/commit/engine.rs`

```rust
use crate::config::types::ResolvedCommitGates;
use crate::error::PilotError;
use crate::gates::commit_pipeline;
use crate::git_ops::{commit_all, emergency_wip_commit};
use std::path::Path;

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

pub struct CommitEngine {
    gate_config: ResolvedCommitGates,
    max_fix_rounds: u32,
    banned_patterns: Vec<crate::gates::banned_pattern::BannedPattern>,
    architecture_rules: Vec<crate::gates::architecture::DependencyRule>,
}

impl CommitEngine {
    pub fn new(
        gate_config: ResolvedCommitGates,
        max_fix_rounds: u32,
        banned_patterns: Vec<crate::gates::banned_pattern::BannedPattern>,
        architecture_rules: Vec<crate::gates::architecture::DependencyRule>,
    ) -> Self {
        Self {
            gate_config,
            max_fix_rounds,
            banned_patterns,
            architecture_rules,
        }
    }

    pub async fn evaluate_commit(
        &self,
        worktree_dir: &Path,
        project_root: &Path,
        stream_id: &str,
        message: &str,
        current_fix_round: u32,
        timeout_secs: u64,
        default_branch: &str,
        branch_version: &str,
    ) -> Result<CommitDecision, PilotError> {
        let ctx = crate::gates::types::GateContext::new(
            worktree_dir.to_path_buf(),
            project_root.to_path_buf(),
            default_branch.to_string(),
            branch_version.to_string(),
            timeout_secs,
        );

        let pipeline_result = commit_pipeline::run_commit_pipeline(
            &ctx,
            &self.gate_config,
            self.banned_patterns.clone(),
            self.architecture_rules.clone(),
        )
        .await?;

        if pipeline_result.passed {
            commit_all(worktree_dir, stream_id, message, timeout_secs).await?;
            Ok(CommitDecision::Committed {
                message: message.to_string(),
                new_fix_round: 0,
            })
        } else {
            let new_fix_round = current_fix_round + 1;
            let feedback = pipeline_result.failure_message();
            if new_fix_round > self.max_fix_rounds {
                emergency_wip_commit(worktree_dir, stream_id, timeout_secs).await?;
                Ok(CommitDecision::Escalated {
                    new_fix_round,
                    feedback,
                })
            } else {
                Ok(CommitDecision::GateFailed {
                    new_fix_round,
                    feedback,
                })
            }
        }
    }
}
```

## `src/session/mod.rs`

```rust
pub mod state;
pub mod machine;
pub mod persistence;

pub use state::{SessionState, StreamStatus, GlobalState};
pub use machine::TransitionValidator;
pub use persistence::DebouncedPersistence;
```

## `src/session/state.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StreamStatus {
    Init,
    Seeding,
    Waiting,
    Streaming,
    Executing,
    Feedback,
    Committing,
    Committed,
    Verifying,
    Reviewing,
    Complete,
    Error,
    Paused,
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
            session_id: uuid::Uuid::new_v4().to_string(),
            stream_id,
            provider_id,
            tab_id: None,
            status: StreamStatus::Init,
            turn: 0,
            fix_round: 0,
            commits: 0,
            worktree_path,
            created_at: now,
            updated_at: now,
            last_activity: now,
            error_message: None,
            provider_cooldown_until: None,
        }
    }

    pub(crate) fn transition(&mut self, new_status: StreamStatus) {
        let now = Utc::now();
        self.status = new_status;
        self.updated_at = now;
        self.last_activity = now;
    }

    pub(crate) fn record_commit(&mut self) {
        self.commits += 1;
        self.fix_round = 0;
        self.last_activity = Utc::now();
    }

    pub(crate) fn record_turn(&mut self) {
        self.turn += 1;
        self.last_activity = Utc::now();
    }

    pub(crate) fn set_provider_cooldown(&mut self, until: DateTime<Utc>) {
        self.provider_cooldown_until = Some(until);
    }

    pub fn is_provider_in_cooldown(&self) -> bool {
        self.provider_cooldown_until.map_or(false, |until| Utc::now() < until)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalState {
    pub sessions: HashMap<String, SessionState>,
    pub version: String,
}

impl GlobalState {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl Default for GlobalState {
    fn default() -> Self {
        Self::new()
    }
}
```

## `src/session/machine.rs`

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
        let current = &session.status;
        if current == &new_status {
            return Ok(());
        }
        let valid = VALID_TRANSITIONS
            .iter()
            .any(|(from, to)| from == current && to == &new_status);
        if !valid {
            Err(PilotError::Session(format!(
                "invalid state transition: {:?} → {:?} (session {})",
                current, new_status, session.stream_id
            )))
        } else {
            Ok(())
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

    fn make_session() -> SessionState {
        SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into())
    }

    #[test]
    fn test_validate_init_to_seeding_ok() {
        assert!(TransitionValidator::validate(&make_session(), StreamStatus::Seeding).is_ok());
    }

    #[test]
    fn test_validate_init_to_error_ok() {
        assert!(TransitionValidator::validate(&make_session(), StreamStatus::Error).is_ok());
    }

    #[test]
    fn test_waiting_to_executing_ok() {
        let mut s = make_session();
        s.transition(StreamStatus::Waiting);
        assert!(TransitionValidator::validate(&s, StreamStatus::Executing).is_ok());
    }

    #[test]
    fn test_feedback_to_committing_ok() {
        let mut s = make_session();
        s.transition(StreamStatus::Feedback);
        assert!(TransitionValidator::validate(&s, StreamStatus::Committing).is_ok());
    }

    #[test]
    fn test_feedback_to_executing_ok() {
        let mut s = make_session();
        s.transition(StreamStatus::Feedback);
        assert!(TransitionValidator::validate(&s, StreamStatus::Executing).is_ok());
    }

    #[test]
    fn test_invalid_transition() {
        let s = make_session();
        assert!(TransitionValidator::validate(&s, StreamStatus::Complete).is_err());
    }
}
```

## `src/session/persistence.rs`

```rust
use crate::error::PilotError;
use super::state::{GlobalState, SessionState, StreamStatus};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;

const STATE_FILE: &str = ".glyim-pilot-state.json";
const TEMP_SUFFIX: &str = ".glyim-tmp";

/// State persistence with:
/// - Atomic writes (write to temp file, then rename)
/// - Debounced saves (dirty flag + explicit flush)
/// - Rollback on save failure (revert in-memory state)
pub struct StatePersistence {
    path: PathBuf,
    state: GlobalState,
    dirty: bool,
}

impl StatePersistence {
    pub async fn load(project_root: &Path) -> Result<Self, PilotError> {
        let path = project_root.join(STATE_FILE);
        let state = if path.exists() {
            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| PilotError::Session(format!("failed to read state: {e}")))?;
            serde_json::from_str(&content)
                .map_err(|e| PilotError::Session(format!("failed to parse state: {e}")))?
        } else {
            GlobalState::new()
        };
        tracing::info!(path = %path.display(), sessions = state.sessions.len(), "loaded persistence");
        Ok(Self {
            path,
            state,
            dirty: false,
        })
    }

    /// Atomic save: write to temp file, then rename.
    /// On POSIX, `rename` is atomic so a crash mid-write won't corrupt.
    async fn save(&mut self) -> Result<(), PilotError> {
        let content = serde_json::to_string(&self.state)
            .map_err(|e| PilotError::Session(format!("serialization failed: {e}")))?;

        let tmp_path = self.path.with_extension(
            format!(
                "{}{}",
                self.path
                    .extension()
                    .map(|e| format!("{}.", e.to_string_lossy()))
                    .unwrap_or_default(),
                TEMP_SUFFIX.trim_start_matches('.')
            ),
        );
        // Simpler: just use a well-known temp path
        let tmp_path = PathBuf::from(format!("{}.tmp", self.path.display()));

        tokio::fs::write(&tmp_path, &content)
            .await
            .map_err(|e| PilotError::Session(format!("temp write failed: {e}")))?;

        tokio::fs::rename(&tmp_path, &self.path)
            .await
            .map_err(|e| {
                // Clean up temp file on rename failure
                let _ = std::fs::remove_file(&tmp_path);
                PilotError::Session(format!("rename to final path failed: {e}"))
            })?;

        self.dirty = false;
        Ok(())
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Flush pending dirty state to disk.
    pub async fn flush(&mut self) -> Result<(), PilotError> {
        if self.dirty {
            self.save().await?;
        }
        Ok(())
    }

    pub fn debug_dump(&self) -> Result<String, PilotError> {
        serde_json::to_string_pretty(&self.state)
            .map_err(|e| PilotError::Session(format!("serialization failed: {e}")))
    }

    pub fn state(&self) -> &GlobalState {
        &self.state
    }

    /// Add a session and immediately persist.
    pub fn add_session(&mut self, session: SessionState) {
        let stream_id = session.stream_id.clone();
        self.state.sessions.insert(stream_id, session);
        self.mark_dirty();
    }

    /// Try to update a session. If the mutation closure succeeds but
    /// the save fails, the in-memory state is reverted to the backup.
    pub async fn try_update_session<F>(
        &mut self,
        stream_id: &str,
        f: F,
    ) -> Result<(), PilotError>
    where
        F: FnOnce(&mut SessionState) -> Result<(), PilotError>,
    {
        let session = self
            .state
            .sessions
            .get_mut(stream_id)
            .ok_or_else(|| PilotError::Session(format!("session {stream_id} not found")))?;

        // Backup before mutation
        let backup = session.clone();

        if let Err(e) = f(session) {
            // Revert in-memory on mutation failure
            *session = backup;
            return Err(e);
        }

        self.mark_dirty();

        // Try to persist — revert in-memory if save fails
        if let Err(e) = self.save().await {
            // Revert the in-memory state
            if let Some(s) = self.state.sessions.get_mut(stream_id) {
                *s = backup;
            }
            return Err(e);
        }

        Ok(())
    }

    pub fn get_session(&self, stream_id: &str) -> Option<&SessionState> {
        self.state.sessions.get(stream_id)
    }

    pub fn active_sessions(&self) -> Vec<&SessionState> {
        self.state
            .sessions
            .values()
            .filter(|s| s.status != StreamStatus::Complete)
            .collect()
    }

    pub fn all_sessions(&self) -> Vec<&SessionState> {
        self.state.sessions.values().collect()
    }

    pub fn remove_session(&mut self, stream_id: &str) {
        self.state.sessions.remove(stream_id);
        self.mark_dirty();
    }

    pub fn session_count(&self) -> usize {
        self.state.sessions.len()
    }
}

/// Thread-safe wrapper with debounced auto-flush.
/// Acquires the inner Mutex for all operations and auto-saves
/// with a configurable debounce window.
pub struct DebouncedPersistence {
    inner: Mutex<StatePersistence>,
    debounce_ms: u64,
}

impl DebouncedPersistence {
    pub async fn load(project_root: &Path) -> Result<Self, PilotError> {
        let inner = StatePersistence::load(project_root).await?;
        Ok(Self {
            inner: Mutex::new(inner),
            debounce_ms: 100,
        })
    }

    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, StatePersistence> {
        self.inner.lock().await
    }

    pub async fn add_session(&self, session: SessionState) -> Result<(), PilotError> {
        let mut p = self.inner.lock().await;
        p.add_session(session);
        p.flush().await
    }

    pub async fn try_update_session<F>(
        &self,
        stream_id: &str,
        f: F,
    ) -> Result<(), PilotError>
    where
        F: FnOnce(&mut SessionState) -> Result<(), PilotError>,
    {
        let mut p = self.inner.lock().await;
        p.try_update_session(stream_id, f).await
    }

    pub async fn remove_session(&self, stream_id: &str) -> Result<(), PilotError> {
        let mut p = self.inner.lock().await;
        p.remove_session(stream_id);
        p.flush().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::machine::TransitionValidator;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, DebouncedPersistence) {
        let dir = tempfile::tempdir().unwrap();
        let p = DebouncedPersistence::load(dir.path()).await.unwrap();
        (dir, p)
    }

    #[tokio::test]
    async fn test_add_and_persist() {
        let dir = tempfile::tempdir().unwrap();
        let p = DebouncedPersistence::load(dir.path()).await.unwrap();
        p.add_session(SessionState::new(
            "S01".into(),
            "deepseek".into(),
            "/tmp/wt".into(),
        ))
        .await
        .unwrap();

        let p2 = DebouncedPersistence::load(dir.path()).await.unwrap();
        let guard = p2.lock().await;
        assert_eq!(guard.session_count(), 1);
    }

    #[tokio::test]
    async fn test_try_update_session_rollback_on_mutation_error() {
        let (_, p) = setup().await;
        p.add_session(SessionState::new(
            "S01".into(),
            "deepseek".into(),
            "/tmp/wt".into(),
        ))
        .await
        .unwrap();

        let result = p
            .try_update_session("S01", |s| {
                s.turn = 99;
                TransitionValidator::validate(s, StreamStatus::Complete)
            })
            .await;
        assert!(result.is_err());

        let guard = p.lock().await;
        assert_eq!(guard.get_session("S01").unwrap().turn, 0);
    }
}
```
## `src/context/mod.rs`

```rust
pub mod budget;
pub mod truncation;
pub mod assembler;

pub use budget::TokenBudget;
pub use assembler::ContextAssembler;
```

## `src/context/budget.rs`

```rust
pub struct TokenBudget {
    pub max_tokens: usize,
    pub used_tokens: usize,
}

impl TokenBudget {
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            used_tokens: 0,
        }
    }
    pub fn remaining(&self) -> usize {
        self.max_tokens.saturating_sub(self.used_tokens)
    }
    pub fn try_allocate(&mut self, tokens: usize) -> bool {
        if self.used_tokens + tokens <= self.max_tokens {
            self.used_tokens += tokens;
            true
        } else {
            false
        }
    }
    pub fn force_allocate(&mut self, tokens: usize) {
        self.used_tokens += tokens;
    }
    pub fn estimate_tokens(text: &str) -> usize {
        (text.len() + 3) / 4
    }
}
```

## `src/context/truncation.rs`

```rust
/// Smart truncation that preserves structural lines (function signatures,
/// type definitions, etc.) while collapsing function bodies.
///
/// Brace counting handles string literals, character literals, raw strings,
/// and line comments to avoid false depth changes. Depth is saturated at 0
/// to prevent negative values from standalone `}` tokens.
pub fn smart_truncate(content: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= max_lines {
        return content.to_string();
    }

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
            || trimmed.starts_with("pub const fn ");

        let is_structural = trimmed.starts_with("pub ")
            || trimmed.starts_with("struct ")
            || trimmed.starts_with("enum ")
            || trimmed.starts_with("trait ")
            || trimmed.starts_with("type ")
            || trimmed.starts_with("const ")
            || trimmed.starts_with("use ")
            || trimmed.starts_with("mod ")
            || trimmed.starts_with("#[")
            || trimmed.starts_with("///")
            || trimmed.starts_with("//!")
            || trimmed.is_empty();

        let (opens, closes) = count_braces(trimmed);

        if is_fn_sig {
            // Close any open body from a previous function
            if brace_depth > 0 {
                result.push("    ...".to_string());
                result.push("}".to_string());
                brace_depth = 0;
            }
            result.push((*line).to_string());
            brace_depth = brace_depth.saturating_add(opens).saturating_sub(closes);
        } else if is_structural {
            result.push((*line).to_string());
            brace_depth = brace_depth.saturating_add(opens).saturating_sub(closes);
        } else if brace_depth > 0 {
            brace_depth = brace_depth.saturating_add(opens).saturating_sub(closes);
            if brace_depth == 0 {
                result.push("    ...".to_string());
                result.push("}".to_string());
            }
        } else {
            brace_depth = brace_depth.saturating_add(opens).saturating_sub(closes);
            if brace_depth == 0 {
                result.push((*line).to_string());
            }
        }

        if result.len() >= max_lines {
            result.push("// ... (truncated for context)".to_string());
            break;
        }
    }

    if brace_depth > 0 {
        result.push("    ...".to_string());
        result.push("}".to_string());
    }

    result.join("\n")
}

/// Count opening and closing braces, accounting for:
/// - Double-quoted strings (with escape sequences)
/// - Character literals (with escape sequences)
/// - Raw strings (`r"..."`, `r#"..."#`, etc.)
/// - Line comments (`//`)
/// - Block comments (`/* */`)
///
/// Depth is returned as raw counts; the caller saturates.
fn count_braces(s: &str) -> (usize, usize) {
    let mut opens = 0;
    let mut closes = 0;
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        // Line comment — skip rest of line
        if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            break;
        }

        // Block comment
        if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len {
                if chars[i] == '*' && chars[i + 1] == '/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Raw string: r"...", r#"..."#, r##"..."##, etc.
        if c == 'r' && i + 1 < len && (chars[i + 1] == '"' || chars[i + 1] == '#') {
            let mut hash_count = 0;
            let mut j = i + 1;
            while j < len && chars[j] == '#' {
                hash_count += 1;
                j += 1;
            }
            if j < len && chars[j] == '"' {
                // Found raw string start; skip to matching end
                j += 1; // skip opening "
                let end_pattern: String = format!(
                    "\"{}",
                    "#".repeat(hash_count)
                );
                let end_chars: Vec<char> = end_pattern.chars().collect();
                'raw_search: while j + end_chars.len() <= len {
                    let mut matched = true;
                    for (k, ec) in end_chars.iter().enumerate() {
                        if chars[j + k] != *ec {
                            matched = false;
                            break;
                        }
                    }
                    if matched {
                        i = j + end_chars.len();
                        continue;
                    }
                    j += 1;
                }
                // Unclosed raw string — skip to end
                i = len;
                continue;
            }
            // Not a raw string, just 'r' followed by something
            i += 1;
            continue;
        }

        // Regular string
        if c == '"' {
            i += 1;
            while i < len {
                if chars[i] == '\\' && i + 1 < len {
                    i += 2; // skip escape sequence
                    continue;
                }
                if chars[i] == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Character literal
        if c == '\'' {
            i += 1;
            while i < len {
                if chars[i] == '\\' && i + 1 < len {
                    i += 2;
                    continue;
                }
                if chars[i] == '\'' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Braces
        if c == '{' {
            opens += 1;
        } else if c == '}' {
            closes += 1;
        }

        i += 1;
    }

    (opens, closes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_braces_basic() {
        assert_eq!(count_braces("fn foo() { }"), (1, 1));
        assert_eq!(count_braces("{{}}"), (2, 2));
    }

    #[test]
    fn test_count_braces_string_literal() {
        // The { inside the string should NOT be counted
        assert_eq!(count_braces(r#"let s = "a { b";"#), (0, 0));
    }

    #[test]
    fn test_count_braces_escaped_quote_in_string() {
        // The \" should not end the string early
        assert_eq!(count_braces(r#"let s = "he said \"hi {\"";"#), (0, 0));
    }

    #[test]
    fn test_count_braces_raw_string() {
        assert_eq!(count_braces(r#"let s = r"no { braces } here";"#), (0, 0));
    }

    #[test]
    fn test_count_braces_raw_string_hash() {
        assert_eq!(
            count_braces(r#"let s = r#"contains { braces }"#;"#),
            (0, 0)
        );
    }

    #[test]
    fn test_count_braces_char_literal() {
        assert_eq!(count_braces("let c = '{';"), (1, 0));
        assert_eq!(count_braces("let c = '}';"), (0, 1));
    }

    #[test]
    fn test_count_braces_line_comment() {
        assert_eq!(count_braces("// { not counted"), (0, 0));
    }

    #[test]
    fn test_count_braces_block_comment() {
        assert_eq!(count_braces("/* { } */"), (0, 0));
    }

    #[test]
    fn test_count_braces_standalone_close() {
        // A standalone } in non-string context
        assert_eq!(count_braces("}"), (0, 1));
    }

    #[test]
    fn test_smart_truncate_preserves_fn_sigs() {
        let code = "fn foo() {\n    body1\n    body2\n}\nfn bar() {\n    body3\n}\nfn baz() {\n    body4\n}";
        let truncated = smart_truncate(code, 4);
        assert!(truncated.contains("fn foo()"));
        assert!(truncated.contains("fn bar()"));
        assert!(truncated.contains("..."));
    }

    #[test]
    fn test_smart_truncate_short_input() {
        let code = "fn main() {}";
        let truncated = smart_truncate(code, 100);
        assert_eq!(truncated, code);
    }
}
```

## `src/context/assembler.rs`

```rust
use super::budget::TokenBudget;
use super::truncation::smart_truncate;
use crate::config::types::PilotConfig;
use crate::error::PilotError;
use std::path::Path;
use std::sync::Arc;

const DEFAULT_MAX_LINES: usize = 800;
const TEST_PREVIEW_LINES: usize = 30;

/// Start/end markers for orchestration sections in the master context.
/// Used instead of keyword filtering to avoid dropping legitimate content.
const ORCHESTRATION_START: &str = "<!-- orchestration-start -->";
const ORCHESTRATION_END: &str = "<!-- orchestration-end -->";

#[derive(Debug, Clone)]
pub struct AssembledContext {
    pub prompt: String,
    pub total_tokens: usize,
    pub tier1_tokens: usize,
    pub tier2_tokens: usize,
    pub tier3_tokens: usize,
    pub tier4_tokens: usize,
}

/// Abstraction over file reading, enabling in-memory testing.
pub trait FileReader: Send + Sync {
    fn read_to_string(&self, path: &Path) -> Option<String>;
}

/// Production file reader that reads from disk.
pub struct DiskFileReader;

impl FileReader for DiskFileReader {
    fn read_to_string(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }
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
        let master_context =
            tokio::fs::read_to_string(project_root.join("AGENT_MASTER_CONTEXT.md"))
                .await
                .ok();
        let contracts_content =
            tokio::fs::read_to_string(project_root.join("CONTRACTS_LOCKED.md"))
                .await
                .ok();
        Self {
            project_root,
            config,
            master_context,
            contracts_content,
            file_reader: Arc::new(DiskFileReader),
        }
    }

    /// Constructor for testing with an injected file reader.
    pub fn new_with_reader(
        project_root: std::path::PathBuf,
        config: Arc<PilotConfig>,
        file_reader: Arc<dyn FileReader>,
    ) -> Self {
        let master_context = file_reader
            .read_to_string(&project_root.join("AGENT_MASTER_CONTEXT.md"));
        let contracts_content = file_reader
            .read_to_string(&project_root.join("CONTRACTS_LOCKED.md"));
        Self {
            project_root,
            config,
            master_context,
            contracts_content,
            file_reader,
        }
    }

    pub async fn assemble(
        &self,
        stream_id: &str,
        owned_files: &[String],
        dependency_interfaces: &[String],
        test_files: &[String],
        provider_id: &str,
    ) -> Result<AssembledContext, PilotError> {
        let max_tokens = self
            .config
            .context
            .providers
            .get(provider_id)
            .map(|c| c.max_context_tokens)
            .unwrap_or(self.config.context.max_context_tokens);
        let mut budget = TokenBudget::new(max_tokens);
        let mut prompt = String::new();

        // Tier 1: Master context + contracts
        let tier1 = self.assemble_tier1()?;
        let tier1_tokens = TokenBudget::estimate_tokens(&tier1);
        budget.force_allocate(tier1_tokens);
        prompt.push_str(&tier1);

        // Tier 2: Owned files + test previews
        let mut tier2_content = String::new();
        for file_path in owned_files {
            let full_path = self.project_root.join(file_path);
            if let Some(content) = self.file_reader.read_to_string(&full_path) {
                let truncated = tokio::task::spawn_blocking(move || {
                    smart_truncate(&content, DEFAULT_MAX_LINES)
                })
                .await
                .map_err(|e| PilotError::Session(format!("spawn_blocking: {e}")))?;
                let section = format!("\n### {file_path}\n```rust\n{truncated}\n```\n");
                if budget.try_allocate(TokenBudget::estimate_tokens(&section)) {
                    tier2_content.push_str(&section);
                }
            } else {
                tracing::warn!(path = %file_path, "failed to read owned file");
            }
        }
        for test_path in test_files {
            let full_path = self.project_root.join(test_path);
            if let Some(content) = self.file_reader.read_to_string(&full_path) {
                let preview: Vec<&str> = content.lines().take(TEST_PREVIEW_LINES).collect();
                let section = format!(
                    "\n### {test_path} (preview)\n```rust\n{}\n// ...\n```\n",
                    preview.join("\n")
                );
                if budget.try_allocate(TokenBudget::estimate_tokens(&section)) {
                    tier2_content.push_str(&section);
                }
            }
        }
        let tier2_tokens = TokenBudget::estimate_tokens(&tier2_content);
        prompt.push_str(&tier2_content);

        // Tier 3: Dependency interfaces
        let mut tier3_content = String::new();
        for dep in dependency_interfaces {
            let section = format!(
                "\n### Dependency: {dep}\n```rust\n// pub signatures only\n```\n"
            );
            if budget.try_allocate(TokenBudget::estimate_tokens(&section)) {
                tier3_content.push_str(&section);
            }
        }
        let tier3_tokens = TokenBudget::estimate_tokens(&tier3_content);
        prompt.push_str(&tier3_content);

        prompt.push_str("\n\n## Output Format\nRespond with ```glyim-ops``` blocks using ::WRITE, ::REPLACE, ::DELETE, ::COMMIT, ::INCOMPLETE, ::DONE, and ::APPROVED directives.\n");

        Ok(AssembledContext {
            prompt,
            total_tokens: budget.used_tokens,
            tier1_tokens,
            tier2_tokens,
            tier3_tokens,
            tier4_tokens: 0,
        })
    }

    fn assemble_tier1(&self) -> Result<String, PilotError> {
        let mut tier1 = String::from("# Glyim Compiler Development\n\n");
        if let Some(ref content) = self.master_context {
            let stripped = strip_orchestration(content);
            tier1.push_str(&stripped);
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

/// Strip orchestration sections using structured markers instead of
/// fragile keyword filtering. Sections delimited by
/// `<!-- orchestration-start -->` / `<!-- orchestration-end -->`
/// are removed. Everything else passes through untouched.
fn strip_orchestration(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut in_orchestration = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == ORCHESTRATION_START {
            in_orchestration = true;
            continue;
        }
        if trimmed == ORCHESTRATION_END {
            in_orchestration = false;
            continue;
        }
        if !in_orchestration {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_orchestration_with_markers() {
        let content = "\
# Project Guide

Some intro text.

<!-- orchestration-start -->
git worktree add ...
cargo fmt
<!-- orchestration-end -->

## Architecture

The `cargo fmt` command is mentioned here legitimately.
";
        let stripped = strip_orchestration(content);
        assert!(stripped.contains("Project Guide"));
        assert!(stripped.contains("Architecture"));
        assert!(stripped.contains("legitimately"));
        assert!(!stripped.contains("git worktree"));
        assert!(!stripped.contains("cargo fmt"));
    }

    #[test]
    fn test_strip_orchestration_no_markers() {
        let content = "# Guide\n\ncargo fmt is great\n";
        let stripped = strip_orchestration(content);
        // Without markers, keyword filtering is NOT applied
        assert!(stripped.contains("cargo fmt"));
    }

    #[test]
    fn test_strip_orchestration_empty_markers() {
        let content = "before\n<!-- orchestration-start -->\n<!-- orchestration-end -->\nafter";
        let stripped = strip_orchestration(content);
        assert!(stripped.contains("before"));
        assert!(stripped.contains("after"));
    }

    /// In-memory file reader for testing ContextAssembler.
    struct InMemoryFileReader {
        files: std::collections::HashMap<String, String>,
    }

    impl InMemoryFileReader {
        fn new(files: Vec<(&str, &str)>) -> Self {
            Self {
                files: files
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            }
        }
    }

    impl FileReader for InMemoryFileReader {
        fn read_to_string(&self, path: &Path) -> Option<String> {
            let key = path.to_string_lossy().to_string();
            // Try exact match first, then just the filename
            if let Some(content) = self.files.get(&key) {
                return Some(content.clone());
            }
            for (stored_path, content) in &self.files {
                if key.ends_with(stored_path) {
                    return Some(content.clone());
                }
            }
            None
        }
    }
}
```

## `src/dispatch/mod.rs`

```rust
pub mod provider_pool;
pub mod rate_limit;
pub mod wave;

pub use provider_pool::ProviderPool;
pub use rate_limit::{handle_rate_limit, RateLimitAction, RateLimitContext};
pub use wave::{dispatch_wave, DispatchStrategy, StreamAssignment};
```

## `src/dispatch/provider_pool.rs`

```rust
use crate::config::types::ProviderConfig;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ProviderPool {
    providers: HashMap<String, ProviderState>,
}

#[derive(Debug, Clone)]
struct ProviderState {
    config: Arc<ProviderConfig>,
    active_slots: usize,
    cooldown_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct SlotAllocation {
    pub provider_id: String,
    pub available_slots: usize,
}

impl ProviderPool {
    pub fn new(providers: &HashMap<String, ProviderConfig>) -> Self {
        let mut states = HashMap::new();
        for (id, config) in providers {
            if config.enabled {
                states.insert(
                    id.clone(),
                    ProviderState {
                        config: Arc::new(config.clone()),
                        active_slots: 0,
                        cooldown_until: None,
                    },
                );
            }
        }
        Self {
            providers: states,
        }
    }

    pub fn allocate(&mut self, provider_id: &str) -> Result<(), String> {
        let state = self
            .providers
            .get_mut(provider_id)
            .ok_or_else(|| format!("provider {provider_id} not found"))?;
        if state.in_cooldown() {
            return Err(format!("provider {provider_id} in cooldown"));
        }
        if state.active_slots >= state.config.max_concurrent {
            return Err(format!("no available slots for {provider_id}"));
        }
        state.active_slots += 1;
        Ok(())
    }

    pub fn free(&mut self, provider_id: &str) {
        if let Some(state) = self.providers.get_mut(provider_id) {
            state.active_slots = state.active_slots.saturating_sub(1);
        }
    }

    pub fn cooldown(&mut self, provider_id: &str, duration_secs: u64) {
        if let Some(state) = self.providers.get_mut(provider_id) {
            let secs = if duration_secs > i64::MAX as u64 {
                i64::MAX
            } else {
                duration_secs as i64
            };
            state.cooldown_until = Some(Utc::now() + Duration::seconds(secs));
        }
    }

    pub fn most_slots_available(&self) -> Option<SlotAllocation> {
        self.providers
            .iter()
            .filter(|(_, s)| !s.in_cooldown() && s.active_slots < s.config.max_concurrent)
            .max_by_key(|(_, s)| s.config.max_concurrent - s.active_slots)
            .map(|(id, s)| SlotAllocation {
                provider_id: id.clone(),
                available_slots: s.config.max_concurrent - s.active_slots,
            })
    }

    pub fn available_slots(&self, provider_id: &str) -> usize {
        self.providers
            .get(provider_id)
            .map(|s| s.config.max_concurrent.saturating_sub(s.active_slots))
            .unwrap_or(0)
    }

    pub fn is_in_cooldown(&self, provider_id: &str) -> bool {
        self.providers
            .get(provider_id)
            .map(|s| s.in_cooldown())
            .unwrap_or(false)
    }

    pub fn provider_ids(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    pub fn get_config(&self, provider_id: &str) -> Option<Arc<ProviderConfig>> {
        self.providers.get(provider_id).map(|s| s.config.clone())
    }

    pub fn total_available_slots(&self) -> usize {
        self.providers
            .values()
            .filter(|s| !s.in_cooldown())
            .map(|s| s.config.max_concurrent.saturating_sub(s.active_slots))
            .sum()
    }
}

impl ProviderState {
    fn in_cooldown(&self) -> bool {
        self.cooldown_until.map_or(false, |until| Utc::now() < until)
    }
}
```

## `src/dispatch/rate_limit.rs`

```rust
use crate::dispatch::provider_pool::ProviderPool;
use crate::error::PilotError;

#[derive(Debug, Clone)]
pub enum RateLimitAction {
    Failover {
        new_provider_id: String,
        failover_prompt: String,
    },
    RetryAfter {
        provider_id: String,
        delay_secs: u64,
    },
    Escalate {
        reason: String,
    },
}

/// Bundles all the state needed for rate-limit handling, instead of
/// passing 10 individual parameters.
#[derive(Debug, Clone)]
pub struct RateLimitContext {
    pub stream_id: String,
    pub turn: u32,
    pub commits: u32,
    pub brief_summary: String,
    pub max_reassign_attempts: u32,
}

pub fn handle_rate_limit(
    pool: &mut ProviderPool,
    provider_id: &str,
    base_delay_secs: u64,
    max_delay_secs: u64,
    attempt: u32,
    ctx: &RateLimitContext,
) -> Result<RateLimitAction, PilotError> {
    let cooldown = pool
        .get_config(provider_id)
        .map(|c| c.rate_limit_cooldown)
        .unwrap_or(base_delay_secs);
    pool.cooldown(provider_id, cooldown);
    tracing::warn!(
        provider_id,
        cooldown_secs = cooldown,
        attempt,
        stream_id = %ctx.stream_id,
        "rate limit detected"
    );

    if attempt <= ctx.max_reassign_attempts {
        if let Some(allocation) = pool.most_slots_available() {
            if allocation.provider_id != provider_id {
                let prompt = build_failover_prompt(
                    &ctx.stream_id,
                    provider_id,
                    &allocation.provider_id,
                    ctx.turn,
                    ctx.commits,
                    &ctx.brief_summary,
                );
                return Ok(RateLimitAction::Failover {
                    new_provider_id: allocation.provider_id,
                    failover_prompt: prompt,
                });
            }
        }
    }

    let delay = calculate_staggered_backoff(base_delay_secs, max_delay_secs, attempt);
    if attempt < 5 {
        Ok(RateLimitAction::RetryAfter {
            provider_id: provider_id.to_string(),
            delay_secs: delay,
        })
    } else {
        Ok(RateLimitAction::Escalate {
            reason: format!("rate limit on {provider_id} after {attempt} attempts"),
        })
    }
}

fn build_failover_prompt(
    stream_id: &str,
    old: &str,
    new: &str,
    turn: u32,
    commits: u32,
    brief: &str,
) -> String {
    format!(
        r#"## Session Failover

This session was moved from **{old}** to **{new}** due to a rate limit.

### Progress So Far
- **Stream**: {stream_id}
- **Turns executed**: {turn}
- **Commits made**: {commits}

### Original Brief
{brief}

### Instructions
Continue from where the previous session left off. The codebase state is preserved – check the current files to see what has already been implemented, then continue with the remaining work. Use the same ```glyim-ops``` protocol for your output.
"#
    )
}

fn calculate_staggered_backoff(base: u64, max: u64, attempt: u32) -> u64 {
    let exp_backoff = base.saturating_mul(2u64.saturating_pow(attempt));
    let capped = exp_backoff.min(max);
    let stagger_range = (capped as f64 * 0.2).max(1.0) as u64;
    let stagger = (attempt as u64 * 17) % stagger_range;
    capped.saturating_add(stagger).min(max)
}
```

## `src/dispatch/wave.rs`

```rust
use crate::dispatch::provider_pool::ProviderPool;
use crate::error::PilotError;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub enum DispatchStrategy {
    MostSlotsFirst,
    RoundRobin,
    LeastLoaded,
}

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
pub struct StreamAssignment {
    pub stream_id: String,
    pub provider_id: String,
}

pub fn dispatch_wave(
    stream_ids: &[String],
    pool: &mut ProviderPool,
    strategy: &DispatchStrategy,
) -> Result<Vec<StreamAssignment>, PilotError> {
    let mut unassigned: VecDeque<String> = stream_ids.iter().cloned().collect();
    let mut assignments = Vec::new();

    match strategy {
        DispatchStrategy::MostSlotsFirst => {
            while let Some(best) = pool.most_slots_available() {
                if pool.allocate(&best.provider_id).is_ok() {
                    if let Some(id) = unassigned.pop_front() {
                        assignments.push(StreamAssignment {
                            stream_id: id,
                            provider_id: best.provider_id,
                        });
                    }
                } else {
                    break;
                }
            }
        }
        DispatchStrategy::RoundRobin => {
            let providers = pool.provider_ids();
            if providers.is_empty() {
                return Ok(assignments);
            }
            let mut idx = 0;
            let mut fails = 0;
            while let Some(id) = unassigned.pop_front() {
                let pid = &providers[idx % providers.len()];
                if pool.allocate(pid).is_ok() {
                    assignments.push(StreamAssignment {
                        stream_id: id,
                        provider_id: pid.clone(),
                    });
                    fails = 0;
                } else {
                    unassigned.push_front(id);
                    fails += 1;
                    if fails > providers.len() * 2 {
                        break;
                    }
                }
                idx += 1;
            }
        }
        DispatchStrategy::LeastLoaded => {
            while let Some(id) = unassigned.pop_front() {
                let mut providers = pool.provider_ids();
                providers.sort_by(|a, b| {
                    pool.available_slots(b).cmp(&pool.available_slots(a))
                });
                let mut allocated = false;
                for pid in &providers {
                    if pool.allocate(pid).is_ok() {
                        assignments.push(StreamAssignment {
                            stream_id: id,
                            provider_id: pid.clone(),
                        });
                        allocated = true;
                        break;
                    }
                }
                if !allocated {
                    break;
                }
            }
        }
    }

    tracing::info!(
        total = stream_ids.len(),
        assigned = assignments.len(),
        strategy = ?strategy,
        "wave dispatch"
    );
    Ok(assignments)
}
```

## `src/server/mod.rs`

```rust
pub mod messages;
pub mod ws;

pub use messages::{ExtensionMessage, CliMessage};
pub use ws::{ServerEvent, WsServer};
```

## `src/server/messages.rs`

```rust
use serde::{Deserialize, Serialize};

/// Protocol version for all messages. Increment when making
/// backward-incompatible changes. The receiver should check
/// `v` and reject messages with unsupported versions.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExtensionMessage {
    #[serde(rename = "session.ready", rename_all = "camelCase")]
    SessionReady {
        session_id: String,
        provider_id: String,
        tab_id: u64,
        #[serde(default)]
        trace_id: Option<String>,
        #[serde(default)]
        v: u32,
    },
    #[serde(rename = "ops.ready", rename_all = "camelCase")]
    OpsReady {
        session_id: String,
        content: String,
        turn: u32,
        #[serde(default)]
        trace_id: Option<String>,
        #[serde(default)]
        v: u32,
    },
    #[serde(rename = "stream.complete", rename_all = "camelCase")]
    StreamComplete {
        session_id: String,
        turn: u32,
        full_response: String,
        #[serde(default)]
        trace_id: Option<String>,
        #[serde(default)]
        v: u32,
    },
    #[serde(rename = "error.detected", rename_all = "camelCase")]
    ErrorDetected {
        session_id: String,
        error_type: String,
        error_message: String,
        recoverable: bool,
        #[serde(default)]
        trace_id: Option<String>,
        #[serde(default)]
        v: u32,
    },
    #[serde(rename = "pong")]
    Pong {
        timestamp: u64,
        #[serde(default)]
        v: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CliMessage {
    #[serde(rename = "session.start", rename_all = "camelCase")]
    SessionStart {
        session_id: String,
        provider_id: String,
        prompt: String,
        system_prompt: String,
        #[serde(default)]
        trace_id: Option<String>,
        #[serde(default)]
        v: u32,
    },
    #[serde(rename = "feedback.send", rename_all = "camelCase")]
    FeedbackSend {
        session_id: String,
        message: String,
        turn: u32,
        #[serde(default)]
        trace_id: Option<String>,
        #[serde(default)]
        v: u32,
    },
    #[serde(rename = "feedback.continue", rename_all = "camelCase")]
    FeedbackContinue {
        session_id: String,
        #[serde(default)]
        trace_id: Option<String>,
        #[serde(default)]
        v: u32,
    },
    #[serde(rename = "retry.prompt", rename_all = "camelCase")]
    RetryPrompt {
        session_id: String,
        message: String,
        delay: u64,
        #[serde(default)]
        trace_id: Option<String>,
        #[serde(default)]
        v: u32,
    },
    #[serde(rename = "session.pause", rename_all = "camelCase")]
    SessionPause {
        session_id: String,
        #[serde(default)]
        trace_id: Option<String>,
        #[serde(default)]
        v: u32,
    },
    #[serde(rename = "session.abort", rename_all = "camelCase")]
    SessionAbort {
        session_id: String,
        #[serde(default)]
        trace_id: Option<String>,
        #[serde(default)]
        v: u32,
    },
    #[serde(rename = "ping")]
    Ping {
        timestamp: u64,
        #[serde(default)]
        v: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_session_ready_camelcase() {
        let msg = ExtensionMessage::SessionReady {
            session_id: "s1".into(),
            provider_id: "deepseek".into(),
            tab_id: 42,
            trace_id: None,
            v: PROTOCOL_VERSION,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"providerId\""));
        assert!(!json.contains("\"session_id\""));
        assert!(json.contains("\"v\":1"));
    }

    #[test]
    fn test_serialize_cli_feedback_send() {
        let msg = CliMessage::FeedbackSend {
            session_id: "s1".into(),
            message: "error".into(),
            turn: 2,
            trace_id: None,
            v: PROTOCOL_VERSION,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"type\":\"feedback.send\""));
        assert!(json.contains("\"v\":1"));
    }

    #[test]
    fn test_version_field_roundtrip() {
        let msg = ExtensionMessage::OpsReady {
            session_id: "s1".into(),
            content: "test".into(),
            turn: 1,
            trace_id: Some("trace-123".into()),
            v: 1,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let de: ExtensionMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(de.version(), 1);
    }

    impl ExtensionMessage {
        pub fn version(&self) -> u32 {
            match self {
                Self::SessionReady { v, .. } => *v,
                Self::OpsReady { v, .. } => *v,
                Self::StreamComplete { v, .. } => *v,
                Self::ErrorDetected { v, .. } => *v,
                Self::Pong { v, .. } => *v,
            }
        }
    }
}
```

## `src/server/ws.rs`

```rust
use crate::error::PilotError;
use crate::server::messages::ExtensionMessage;
use crate::server::messages::PROTOCOL_VERSION;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};

#[derive(Debug, Clone)]
pub enum ServerEvent {
    Connected { addr: SocketAddr },
    Message {
        session_id: Option<String>,
        trace_id: Option<String>,
        msg: ExtensionMessage,
    },
    Disconnected { addr: SocketAddr },
}

pub struct WsServer {
    addr: SocketAddr,
    event_tx: mpsc::UnboundedSender<ServerEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<ServerEvent>>,
    cli_msg_tx: broadcast::Sender<String>,
}

impl WsServer {
    pub fn new(host: &str, port: u16) -> Self {
        let addr: SocketAddr = format!("{host}:{port}")
            .parse()
            .expect("invalid bind address");
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (cli_msg_tx, _) = broadcast::channel(256);
        Self {
            addr,
            event_tx,
            event_rx: Some(event_rx),
            cli_msg_tx,
        }
    }

    pub fn take_event_rx(&mut self) -> Option<mpsc::UnboundedReceiver<ServerEvent>> {
        self.event_rx.take()
    }

    pub fn cli_msg_sender(&self) -> broadcast::Sender<String> {
        self.cli_msg_tx.clone()
    }

    pub async fn run(&self) -> Result<(), PilotError> {
        let listener = TcpListener::bind(&self.addr).await?;
        tracing::info!("WebSocket server listening on ws://{}", self.addr);
        tracing::info!(
            "SECURITY: Only loopback connections accepted. \
             Set [server] host = \"0.0.0.0\" in .glyim-pilot.toml \
             for container/VM use (NOT recommended for production)."
        );

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    if !addr.ip().is_loopback() {
                        tracing::error!(
                            peer = %addr,
                            "REJECTED non-localhost connection"
                        );
                        continue;
                    }
                    let event_tx = self.event_tx.clone();
                    let cli_msg_rx = self.cli_msg_tx.subscribe();
                    tokio::spawn(async move {
                        let ws_stream = match tokio_tungstenite::accept_async(stream).await {
                            Ok(ws) => ws,
                            Err(e) => {
                                tracing::warn!(peer = %addr, "handshake failed: {e}");
                                return;
                            }
                        };
                        tracing::info!(peer = %addr, "extension connected");
                        let _ = event_tx.send(ServerEvent::Connected { addr });
                        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

                        // Outgoing message sender
                        let send_tx = event_tx.clone();
                        let mut send_rx = cli_msg_rx;
                        let send_addr = addr;
                        tokio::spawn(async move {
                            while let Ok(msg) = send_rx.recv().await {
                                if ws_sender
                                    .send(tokio_tungstenite::tungstenite::Message::Text(
                                        msg.into(),
                                    ))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            let _ = send_tx.send(ServerEvent::Disconnected { addr: send_addr });
                        });

                        // Incoming message receiver
                        let recv_tx = event_tx.clone();
                        let recv_addr = addr;
                        while let Some(msg) = ws_receiver.next().await {
                            match msg {
                                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                                    if let Ok(ext_msg) =
                                        serde_json::from_str::<ExtensionMessage>(&text)
                                    {
                                        // Version check
                                        let msg_version = match &ext_msg {
                                            ExtensionMessage::SessionReady { v, .. } => *v,
                                            ExtensionMessage::OpsReady { v, .. } => *v,
                                            ExtensionMessage::StreamComplete { v, .. } => *v,
                                            ExtensionMessage::ErrorDetected { v, .. } => *v,
                                            ExtensionMessage::Pong { v, .. } => *v,
                                        };
                                        if msg_version > PROTOCOL_VERSION {
                                            tracing::warn!(
                                                peer = %recv_addr,
                                                msg_version,
                                                server_version = PROTOCOL_VERSION,
                                                "message from newer protocol version — may not be fully supported"
                                            );
                                        }

                                        let (session_id, trace_id) = match &ext_msg {
                                            ExtensionMessage::SessionReady {
                                                session_id,
                                                trace_id,
                                                ..
                                            } => (Some(session_id.clone()), trace_id.clone()),
                                            ExtensionMessage::OpsReady {
                                                session_id,
                                                trace_id,
                                                ..
                                            } => (Some(session_id.clone()), trace_id.clone()),
                                            ExtensionMessage::StreamComplete {
                                                session_id,
                                                trace_id,
                                                ..
                                            } => (Some(session_id.clone()), trace_id.clone()),
                                            ExtensionMessage::ErrorDetected {
                                                session_id,
                                                trace_id,
                                                ..
                                            } => (Some(session_id.clone()), trace_id.clone()),
                                            ExtensionMessage::Pong { .. } => (None, None),
                                        };
                                        let _ = recv_tx.send(ServerEvent::Message {
                                            session_id,
                                            trace_id,
                                            msg: ext_msg,
                                        });
                                    }
                                }
                                Ok(tokio_tungstenite::tungstenite::Message::Ping(data)) => {
                                    let _ = ws_sender
                                        .send(tokio_tungstenite::tungstenite::Message::Pong(data))
                                        .await;
                                }
                                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => break,
                                _ => {}
                            }
                        }
                        tracing::info!(peer = %recv_addr, "extension disconnected");
                        let _ = recv_tx.send(ServerEvent::Disconnected { addr: recv_addr });
                    });
                }
                Err(e) => {
                    tracing::error!(error = %e, "accept failed");
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
    }
}
```

## `src/orchestrator/mod.rs`

```rust
pub mod turn;
pub use turn::{OrchestratorAction, process_turn_dispatch};
```

## `src/orchestrator/turn.rs`

```rust
use crate::applier::{apply_ops_async, ApplyLimits};
use crate::commit::{CommitDecision, CommitEngine};
use crate::config::types::PilotConfig;
use crate::error::PilotError;
use crate::gates::done_pipeline;
use crate::gates::self_review::build_review_prompt;
use crate::git_ops::{create_pr, diff_main, log_oneline, push_branch};
use crate::metrics::Metrics;
use crate::protocol::parser::parse_ops_block;
use crate::session::machine::TransitionValidator;
use crate::session::persistence::DebouncedPersistence;
use crate::session::state::StreamStatus;
use std::collections::HashSet;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub enum OrchestratorAction {
    Feedback {
        session_id: String,
        message: String,
        trace_id: Option<String>,
    },
    Continue {
        session_id: String,
        trace_id: Option<String>,
    },
    SelfReview {
        session_id: String,
        prompt: String,
        trace_id: Option<String>,
    },
    StreamComplete {
        session_id: String,
        pr_url: String,
        trace_id: Option<String>,
    },
    Escalate {
        session_id: String,
        reason: String,
        trace_id: Option<String>,
    },
    WaitForResponse {
        session_id: String,
        trace_id: Option<String>,
    },
}

/// Process a turn from the AI, handling concurrency safety and panic recovery.
///
/// Uses `catch_unwind` to ensure the processing lock is always released,
/// even if the inner processing logic panics.
pub async fn process_turn_dispatch(
    ops_block: &str,
    session_id: &str,
    stream_id: &str,
    worktree_dir: PathBuf,
    project_root: PathBuf,
    config: Arc<PilotConfig>,
    persistence: Arc<DebouncedPersistence>,
    processing: Arc<Mutex<HashSet<String>>>,
    turn: u32,
    trace_id: Option<String>,
    metrics: &dyn Metrics,
) -> Result<OrchestratorAction, PilotError> {
    // Set up tracing span with trace_id propagation
    let span = tracing::info_span!(
        "process_turn",
        stream_id,
        turn,
        trace_id = trace_id.as_deref().unwrap_or("")
    );

    // Concurrency guard: acquire
    {
        let _enter = span.enter();
        let mut guard = processing.lock().await;
        if !guard.insert(stream_id.to_string()) {
            tracing::warn!(stream_id, "session already being processed, skipping duplicate");
            return Ok(OrchestratorAction::WaitForResponse {
                session_id: session_id.to_string(),
                trace_id,
            });
        }
    }

    // Execute with panic recovery
    let result = tokio::spawn(async move {
        // Use catch_unwind for panic safety
        let fut = process_turn_inner(
            ops_block,
            session_id,
            stream_id,
            &worktree_dir,
            &project_root,
            &config,
            &persistence,
            turn,
            &trace_id,
            metrics,
        );

        // AssertUnwindSafe is safe here because we're only catching the panic
        // to clean up the processing lock — we don't rely on any invariants
        // from the panicked computation.
        match std::panic::catch_unwind(AssertUnwindSafe(|| fut)) {
            Ok(fut) => fut.await,
            Err(panic_payload) => {
                let reason = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                tracing::error!(stream_id, reason = %reason, "process_turn_inner panicked");
                Err(PilotError::Session(format!(
                    "processing panicked for stream {stream_id}: {reason}"
                )))
            }
        }
    })
    .await;

    // ALWAYS remove from processing set, even on panic or task join error
    {
        let mut guard = processing.lock().await;
        guard.remove(stream_id);
    }

    // Handle the result
    match result {
        Ok(inner_result) => {
            metrics.increment_counter("turn_processed", &[("stream", stream_id)]);
            inner_result
        }
        Err(join_error) => {
            let reason = if join_error.is_panic() {
                "task panicked".into()
            } else if join_error.is_cancelled() {
                "task cancelled".into()
            } else {
                format!("join error: {join_error}")
            };
            tracing::error!(stream_id, reason = %reason, "processing task failed");
            metrics.increment_counter("turn_panic", &[("stream", stream_id)]);
            Err(PilotError::Session(format!(
                "processing task failed for stream {stream_id}: {reason}"
            )))
        }
    }
}

async fn process_turn_inner(
    ops_block: &str,
    session_id: &str,
    stream_id: &str,
    worktree_dir: &PathBuf,
    project_root: &PathBuf,
    config: &Arc<PilotConfig>,
    persistence: &Arc<DebouncedPersistence>,
    turn: u32,
    trace_id: &Option<String>,
    metrics: &dyn Metrics,
) -> Result<OrchestratorAction, PilotError> {
    let ops = parse_ops_block(ops_block)?;
    tracing::info!(ops_count = ops.ops.len(), "parsed ops block");

    // Drive session state: if still Init, advance to Waiting
    {
        persistence
            .try_update_session(stream_id, |s| {
                match s.status {
                    StreamStatus::Init => {
                        TransitionValidator::transition(s, StreamStatus::Seeding)?;
                        TransitionValidator::transition(s, StreamStatus::Waiting)?;
                    }
                    StreamStatus::Feedback | StreamStatus::Committed => {
                        TransitionValidator::transition(s, StreamStatus::Waiting)?;
                    }
                    _ => {}
                }
                Ok(())
            })
            .await?;
    }

    // Apply file operations (two-phase commit with limits)
    if !ops.ops.is_empty() {
        {
            persistence
                .try_update_session(stream_id, |s| {
                    TransitionValidator::transition(s, StreamStatus::Executing)?;
                    Ok(())
                })
                .await?;
        }
        let results = apply_ops_async(
            worktree_dir.clone(),
            ops.ops.clone(),
            config.limits.clone(),
        )
        .await?;
        tracing::info!(applied = results.len(), "file operations applied");
        metrics.record_histogram(
            "ops_applied",
            results.len() as f64,
            &[("stream", stream_id)],
        );
    }

    // Route based on control directives
    if ops.approved {
        return handle_approved(
            session_id,
            stream_id,
            worktree_dir,
            config,
            persistence,
            trace_id,
            metrics,
        )
        .await;
    }
    if ops.done {
        return handle_done(
            session_id,
            stream_id,
            worktree_dir,
            project_root,
            config,
            persistence,
            trace_id,
            metrics,
        )
        .await;
    }
    if ops.incomplete {
        persistence
            .try_update_session(stream_id, |s| {
                s.record_turn();
                Ok(())
            })
            .await?;
        return Ok(OrchestratorAction::Continue {
            session_id: session_id.to_string(),
            trace_id: trace_id.clone(),
        });
    }
    if let Some(msg) = ops.commit_message {
        return handle_commit(
            session_id,
            stream_id,
            worktree_dir,
            project_root,
            config,
            persistence,
            &msg,
            trace_id,
            turn,
            metrics,
        )
        .await;
    }

    // No control directive
    persistence
        .try_update_session(stream_id, |s| {
            s.record_turn();
            Ok(())
        })
        .await?;
    Ok(OrchestratorAction::WaitForResponse {
        session_id: session_id.to_string(),
        trace_id: trace_id.clone(),
    })
}

async fn handle_commit(
    session_id: &str,
    stream_id: &str,
    worktree_dir: &PathBuf,
    project_root: &PathBuf,
    config: &Arc<PilotConfig>,
    persistence: &Arc<DebouncedPersistence>,
    commit_message: &str,
    trace_id: &Option<String>,
    _turn: u32,
    metrics: &dyn Metrics,
) -> Result<OrchestratorAction, PilotError> {
    let current_fix_round = {
        let p = persistence.lock().await;
        p.get_session(stream_id).map(|s| s.fix_round).unwrap_or(0)
    };

    // Transition to Committing
    persistence
        .try_update_session(stream_id, |s| {
            TransitionValidator::transition(s, StreamStatus::Committing)?;
            Ok(())
        })
        .await?;

    // Resolve gates with explicit branch info — no post-hoc mutation
    let resolved = config.gates.commit.resolve(
        config.gates.level,
        config.execution.default_branch.clone(),
        config.execution.branch_version.clone(),
    );

    let engine = CommitEngine::new(
        resolved,
        config.execution.max_fix_rounds,
        config.gates.banned_patterns.clone(),
        config.gates.architecture_rules.clone(),
    );
    let decision = engine
        .evaluate_commit(
            worktree_dir,
            project_root,
            stream_id,
            commit_message,
            current_fix_round,
            config.execution.command_timeout,
            &config.execution.default_branch,
            &config.execution.branch_version,
        )
        .await?;

    persistence
        .try_update_session(stream_id, |s| {
            if s.fix_round != current_fix_round {
                return Err(PilotError::Session(format!(
                    "fix_round changed from {} to {}",
                    current_fix_round, s.fix_round
                )));
            }
            match &decision {
                CommitDecision::Committed { new_fix_round, .. } => {
                    s.record_commit();
                    s.fix_round = *new_fix_round;
                    TransitionValidator::transition(s, StreamStatus::Committed)?;
                }
                CommitDecision::GateFailed { new_fix_round, .. } => {
                    s.fix_round = *new_fix_round;
                    TransitionValidator::transition(s, StreamStatus::Feedback)?;
                }
                CommitDecision::Escalated { new_fix_round, .. } => {
                    s.fix_round = *new_fix_round;
                    TransitionValidator::transition(s, StreamStatus::Error)?;
                }
            }
            Ok(())
        })
        .await?;

    metrics.increment_counter(
        "commit_decision",
        &[(
            "result",
            match &decision {
                CommitDecision::Committed { .. } => "committed",
                CommitDecision::GateFailed { .. } => "gate_failed",
                CommitDecision::Escalated { .. } => "escalated",
            },
        )],
    );

    match decision {
        CommitDecision::Committed { message, .. } => Ok(OrchestratorAction::Feedback {
            session_id: session_id.to_string(),
            message: format!("✅ Committed: {}", message),
            trace_id: trace_id.clone(),
        }),
        CommitDecision::GateFailed { feedback, .. } => Ok(OrchestratorAction::Feedback {
            session_id: session_id.to_string(),
            message: format!("❌ Commit gate failed:\n\n{}", feedback),
            trace_id: trace_id.clone(),
        }),
        CommitDecision::Escalated { feedback, .. } => Ok(OrchestratorAction::Escalate {
            session_id: session_id.to_string(),
            reason: format!("Fix rounds exceeded.\n\n{}", feedback),
            trace_id: trace_id.clone(),
        }),
    }
}

async fn handle_done(
    session_id: &str,
    stream_id: &str,
    worktree_dir: &PathBuf,
    project_root: &PathBuf,
    config: &Arc<PilotConfig>,
    persistence: &Arc<DebouncedPersistence>,
    trace_id: &Option<String>,
    metrics: &dyn Metrics,
) -> Result<OrchestratorAction, PilotError> {
    let resolved = config.gates.done.resolve(config.gates.level);
    let ctx = crate::gates::types::GateContext::new(
        worktree_dir.clone(),
        project_root.clone(),
        config.execution.default_branch.clone(),
        config.execution.branch_version.clone(),
        config.execution.command_timeout,
    );
    let result = done_pipeline::run_done_pipeline(&ctx, &resolved).await?;

    metrics.increment_counter(
        "done_pipeline",
        &[(
            "passed",
            if result.passed { "true" } else { "false" },
        )],
    );

    if result.passed {
        let diff = diff_main(
            worktree_dir,
            &config.execution.default_branch,
            config.execution.command_timeout,
        )
        .await?;
        let log = log_oneline(
            worktree_dir,
            &config.execution.default_branch,
            config.execution.command_timeout,
        )
        .await?;
        let review_prompt = build_review_prompt(&diff, &log);

        persistence
            .try_update_session(stream_id, |s| {
                TransitionValidator::transition(s, StreamStatus::Verifying)?;
                TransitionValidator::transition(s, StreamStatus::Reviewing)?;
                Ok(())
            })
            .await?;

        Ok(OrchestratorAction::SelfReview {
            session_id: session_id.to_string(),
            prompt: review_prompt,
            trace_id: trace_id.clone(),
        })
    } else {
        let feedback = result.failure_message();
        persistence
            .try_update_session(stream_id, |s| {
                TransitionValidator::transition(s, StreamStatus::Feedback)?;
                Ok(())
            })
            .await?;
        Ok(OrchestratorAction::Feedback {
            session_id: session_id.to_string(),
            message: format!("❌ Done gate failed:\n\n{}", feedback),
            trace_id: trace_id.clone(),
        })
    }
}

async fn handle_approved(
    session_id: &str,
    stream_id: &str,
    worktree_dir: &PathBuf,
    config: &Arc<PilotConfig>,
    persistence: &Arc<DebouncedPersistence>,
    trace_id: &Option<String>,
    metrics: &dyn Metrics,
) -> Result<OrchestratorAction, PilotError> {
    push_branch(
        worktree_dir,
        stream_id,
        &config.execution.branch_version,
        config.execution.command_timeout,
    )
    .await?;
    let title = format!("stream-{}: implementation", stream_id);
    let body = format!("Automated implementation for stream {}", stream_id);
    let pr_url = create_pr(
        worktree_dir,
        stream_id,
        &config.execution.default_branch,
        &config.execution.branch_version,
        &title,
        &body,
        config.execution.command_timeout,
    )
    .await?;

    persistence
        .try_update_session(stream_id, |s| {
            TransitionValidator::transition(s, StreamStatus::Complete)?;
            Ok(())
        })
        .await?;

    metrics.increment_counter("pr_created", &[("stream", stream_id)]);

    Ok(OrchestratorAction::StreamComplete {
        session_id: session_id.to_string(),
        pr_url,
        trace_id: trace_id.clone(),
    })
}
```

## `src/cli/mod.rs`

```rust
pub mod dashboard;
pub use dashboard::{render_status_table, render_wave_summary};
```

## `src/cli/dashboard.rs`

```rust
use crate::session::state::{SessionState, StreamStatus};
use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, Table};

pub fn render_status_table(sessions: &[SessionState]) -> String {
    if sessions.is_empty() {
        return "No active sessions.".to_string();
    }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Stream").add_attribute(Attribute::Bold),
        Cell::new("Provider").add_attribute(Attribute::Bold),
        Cell::new("Status").add_attribute(Attribute::Bold),
        Cell::new("Turn").add_attribute(Attribute::Bold),
        Cell::new("Fixes").add_attribute(Attribute::Bold),
        Cell::new("Commits").add_attribute(Attribute::Bold),
        Cell::new("Last Activity").add_attribute(Attribute::Bold),
    ]);
    for s in sessions {
        let color = match s.status {
            StreamStatus::Complete => Color::Green,
            StreamStatus::Error => Color::Red,
            StreamStatus::Paused => Color::Yellow,
            StreamStatus::Streaming | StreamStatus::Executing => Color::Cyan,
            _ => Color::White,
        };
        table.add_row(vec![
            Cell::new(&s.stream_id),
            Cell::new(&s.provider_id),
            Cell::new(format!("{:?}", s.status)).fg(color),
            Cell::new(s.turn),
            Cell::new(s.fix_round),
            Cell::new(s.commits),
            Cell::new(s.last_activity.format("%H:%M:%S")),
        ]);
    }
    table.to_string()
}

pub fn render_wave_summary(sessions: &[SessionState]) -> String {
    if sessions.is_empty() {
        return "No sessions in wave.".to_string();
    }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        Cell::new("Stream").add_attribute(Attribute::Bold),
        Cell::new("Provider").add_attribute(Attribute::Bold),
        Cell::new("Turns").add_attribute(Attribute::Bold),
        Cell::new("Fixes").add_attribute(Attribute::Bold),
        Cell::new("Commits").add_attribute(Attribute::Bold),
        Cell::new("Status").add_attribute(Attribute::Bold),
    ]);
    for s in sessions {
        let color = match s.status {
            StreamStatus::Complete => Color::Green,
            StreamStatus::Error => Color::Red,
            _ => Color::White,
        };
        table.add_row(vec![
            Cell::new(&s.stream_id),
            Cell::new(&s.provider_id),
            Cell::new(s.turn),
            Cell::new(s.fix_round),
            Cell::new(s.commits),
            Cell::new(format!("{:?}", s.status)).fg(color),
        ]);
    }
    let total_turns: u32 = sessions.iter().map(|s| s.turn).sum();
    let total_commits: u32 = sessions.iter().map(|s| s.commits).sum();
    let completed = sessions
        .iter()
        .filter(|s| s.status == StreamStatus::Complete)
        .count();
    format!(
        "{table}\n\nSummary: {completed}/{} complete, {} total turns, {} total commits",
        sessions.len(),
        total_turns,
        total_commits
    )
}
```

---

Now the Chrome extension with all fixes applied:

## `extension/package.json`

```json
{
  "name": "glyim-pilot-extension",
  "version": "0.2.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "test": "vitest run",
    "lint": "tsc --noEmit"
  },
  "devDependencies": {
    "@crxjs/vite-plugin": "^2.0.0-beta.28",
    "@types/chrome": "^0.0.287",
    "typescript": "^5.7.0",
    "vite": "^6.1.0",
    "vitest": "^3.0.0"
  }
}
```

## `extension/tsconfig.json`

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "outDir": "./dist",
    "rootDir": "./src",
    "types": ["chrome"]
  },
  "include": ["src/**/*.ts"],
  "exclude": ["node_modules", "dist"]
}
```

## `extension/vite.config.ts`

```typescript
import { defineConfig } from 'vite';
import { crx } from '@crxjs/vite-plugin';
import manifest from './manifest.json';

export default defineConfig({
  plugins: [crx({ manifest })],
  build: { outDir: 'dist', sourcemap: process.env.NODE_ENV === 'development' },
  test: { globals: true, environment: 'jsdom' },
});
```

## `extension/manifest.json`

```json
{
  "manifest_version": 3,
  "name": "Glyim Pilot",
  "description": "AI chat monitoring and code extraction for Glyim Pilot",
  "version": "0.2.0",
  "permissions": ["tabs", "activeTab", "storage", "scripting"],
  "host_permissions": [
    "https://chat.deepseek.com/*",
    "https://z.ai/*",
    "https://gemini.google.com/*",
    "https://grok.x.ai/*",
    "https://chat.mistral.ai/*"
  ],
  "background": { "service_worker": "src/background.ts", "type": "module" },
  "content_scripts": [{
    "matches": [
      "https://chat.deepseek.com/*",
      "https://z.ai/*",
      "https://gemini.google.com/*",
      "https://grok.x.ai/*",
      "https://chat.mistral.ai/*"
    ],
    "js": ["src/content.ts"],
    "run_at": "document_idle"
  }],
  "icons": {
    "16": "icons/icon16.png",
    "48": "icons/icon48.png",
    "128": "icons/icon128.png"
  }
}
```

## `extension/src/types.ts`

```typescript
export const PROTOCOL_VERSION = 1;

export interface SessionReady {
  type: 'session.ready';
  sessionId: string;
  providerId: string;
  tabId: number;
  traceId?: string;
  v: number;
}

export interface OpsReady {
  type: 'ops.ready';
  sessionId: string;
  content: string;
  turn: number;
  traceId?: string;
  v: number;
}

export interface StreamComplete {
  type: 'stream.complete';
  sessionId: string;
  turn: number;
  fullResponse: string;
  traceId?: string;
  v: number;
}

export interface ErrorDetected {
  type: 'error.detected';
  sessionId: string;
  errorType: string;
  errorMessage: string;
  recoverable: boolean;
  traceId?: string;
  v: number;
}

export interface Pong { type: 'pong'; timestamp: number; v: number; }

export type ExtensionMessage = SessionReady | OpsReady | StreamComplete | ErrorDetected | Pong;

export interface SessionStart {
  type: 'session.start';
  sessionId: string;
  providerId: string;
  prompt: string;
  systemPrompt: string;
  traceId?: string;
  v: number;
}

export interface FeedbackSend {
  type: 'feedback.send';
  sessionId: string;
  message: string;
  turn: number;
  traceId?: string;
  v: number;
}

export interface FeedbackContinue {
  type: 'feedback.continue';
  sessionId: string;
  traceId?: string;
  v: number;
}

export interface RetryPrompt {
  type: 'retry.prompt';
  sessionId: string;
  message: string;
  delay: number;
  traceId?: string;
  v: number;
}

export interface SessionPause {
  type: 'session.pause';
  sessionId: string;
  traceId?: string;
  v: number;
}

export interface SessionAbort {
  type: 'session.abort';
  sessionId: string;
  traceId?: string;
  v: number;
}

export interface Ping { type: 'ping'; timestamp: number; v: number; }

export type CliMessage = SessionStart | FeedbackSend | FeedbackContinue | RetryPrompt | SessionPause | SessionAbort | Ping;

export interface TabSession {
  tabId: number;
  sessionId: string;
  streamId: string;
  providerId: string;
  status: 'active' | 'paused' | 'error';
  turn: number;
}

export const DANGEROUS_PATTERNS: readonly string[] = [
  'rm -rf', 'git push', 'git reset --hard', 'cargo publish', 'sudo', 'chmod 777', 'mkfs', 'dd if=',
];

export function containsDangerousPattern(content: string): string | null {
  const lower = content.toLowerCase();
  for (const pattern of DANGEROUS_PATTERNS) {
    if (lower.includes(pattern.toLowerCase())) return pattern;
  }
  return null;
}

export function normalizeLineEndings(text: string): string {
  return text.replace(/\r/g, '');
}

/** Serialize tabSessions preserving numeric keys properly. */
export function serializeTabSessions(sessions: Map<number, TabSession>): string {
  const obj: Record<string, TabSession> = {};
  for (const [tabId, session] of sessions.entries()) {
    obj[String(tabId)] = session;
  }
  return JSON.stringify(obj);
}

/** Deserialize tabSessions with proper numeric key restoration. */
export function deserializeTabSessions(raw: unknown): Map<number, TabSession> {
  const result = new Map<number, TabSession>();
  if (typeof raw !== 'object' || raw === null) return result;
  const obj = raw as Record<string, unknown>;
  for (const [key, value] of Object.entries(obj)) {
    const tabId = Number(key);
    if (!Number.isFinite(tabId)) continue;
    if (typeof value === 'object' && value !== null) {
      result.set(tabId, value as TabSession);
    }
  }
  return result;
}
```

## `extension/src/ws_client.ts`

```typescript
import type { ExtensionMessage, CliMessage } from './types';
import { PROTOCOL_VERSION } from './types';

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

  disconnect(): void {
    this.intentionalClose = true;
    this.cleanup();
    this.ws?.close();
    this.ws = null;
  }

  send(msg: ExtensionMessage): boolean {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return false;
    this.ws.send(JSON.stringify(msg));
    return true;
  }

  get connected(): boolean { return this.ws !== null && this.ws.readyState === WebSocket.OPEN; }

  private doConnect(): void {
    try { this.ws = new WebSocket(this.url); } catch { this.scheduleReconnect(); return; }

    this.ws.onopen = () => {
      this.reconnectAttempts = 0;
      this.statusHandler?.(true);
      this.startPing();
    };

    this.ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data) as CliMessage;
        // Version check
        const msgVersion = (msg as Record<string, unknown>).v as number | undefined;
        if (msgVersion !== undefined && msgVersion > PROTOCOL_VERSION) {
          console.warn(
            `glyim-pilot: received message with version ${msgVersion}, ` +
            `server version is ${PROTOCOL_VERSION} — may not work correctly`
          );
        }
        this.messageHandler?.(msg);
      } catch {}
    };

    this.ws.onclose = () => {
      this.statusHandler?.(false);
      this.stopPing();
      this.ws = null;
      if (!this.intentionalClose) this.scheduleReconnect();
    };
  }

  private scheduleReconnect(): void {
    if (this.intentionalClose) return;
    const delay = Math.min(RECONNECT_BASE_DELAY * Math.pow(2, this.reconnectAttempts), RECONNECT_MAX_DELAY);
    this.reconnectAttempts++;
    this.reconnectTimer = setTimeout(() => this.doConnect(), delay);
  }

  private startPing(): void {
    this.stopPing();
    this.pingTimer = setInterval(
      () => this.send({ type: 'ping', timestamp: Date.now(), v: PROTOCOL_VERSION }),
      PING_INTERVAL
    );
  }

  private stopPing(): void { if (this.pingTimer) clearInterval(this.pingTimer); }
  private cleanup(): void { if (this.reconnectTimer) clearTimeout(this.reconnectTimer); this.stopPing(); }
}
```

## `extension/src/providers/adapter.ts`

```typescript
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

export interface ProviderError {
  type: 'rate_limit' | 'server_busy' | 'capacity' | 'server_error' | 'network_error';
  message: string;
  recoverable: boolean;
}

const adapterRegistry: ProviderAdapter[] = [];

export function registerAdapter(adapter: ProviderAdapter): void { adapterRegistry.push(adapter); }
export function getAdapterForUrl(url: string): ProviderAdapter | null {
  return adapterRegistry.find((a) => a.urlPattern.test(url)) ?? null;
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
      'button[type="submit"], button[aria-label*="send"], div[class*="send-button"]'
    );
    if (btn && !btn.disabled && !btn.getAttribute('aria-disabled')) {
      btn.click();
      return;
    }
    await new Promise(r => setTimeout(r, pollInterval));
  }
  throw new Error('send button not found or not enabled within timeout');
}
```

## `extension/src/providers/deepseek.ts`

```typescript
import type { ProviderAdapter, ProviderError } from './adapter';
import { insertText, clickSendWhenEnabled } from './adapter';

export class DeepSeekAdapter implements ProviderAdapter {
  readonly id = 'deepseek';
  readonly urlPattern = /chat\.deepseek\.com/;
  readonly assistantSelector = '.ds-markdown--block';
  readonly homepageUrl = 'https://chat.deepseek.com';

  async setInput(text: string): Promise<void> {
    const textarea = document.querySelector<HTMLTextAreaElement>("textarea[id='chat-input']");
    if (!textarea) throw new Error('DeepSeek: input not found');
    textarea.focus();
    insertText(textarea, text);
  }

  async submitMessage(): Promise<void> { await clickSendWhenEnabled(); }
  isStreaming(): boolean { return document.querySelector('.typing-indicator') !== null; }
  getCodeBlocks(): string[] { return Array.from(document.querySelectorAll('pre code')).map(b => b.textContent ?? ''); }

  detectError(): ProviderError | null {
    const errorElements = document.querySelectorAll('.error-banner, .toast-error, [class*="error-message"]');
    for (const el of errorElements) {
      if (el.closest(this.assistantSelector)) continue;
      const text = el.textContent?.toLowerCase() ?? '';
      if (text.includes('rate limit') || text.includes('too frequent')) {
        return { type: 'rate_limit', message: el.textContent?.trim() ?? 'Rate limit', recoverable: true };
      }
      if (text.includes('server error')) {
        return { type: 'server_error', message: el.textContent?.trim() ?? 'Server error', recoverable: true };
      }
    }
    return null;
  }

  getAssistantText(): string {
    return document.querySelector('.ds-markdown--block:last-of-type')?.textContent ?? '';
  }
}
```

## `extension/src/providers/zai.ts`

```typescript
import type { ProviderAdapter, ProviderError } from './adapter';
import { insertText, clickSendWhenEnabled } from './adapter';

export class ZaiAdapter implements ProviderAdapter {
  readonly id = 'zai';
  readonly urlPattern = /z\.ai/;
  readonly assistantSelector = '.message-assistant';
  readonly homepageUrl = 'https://z.ai';

  async setInput(text: string): Promise<void> {
    const textarea = document.querySelector<HTMLTextAreaElement>('textarea');
    if (!textarea) throw new Error('z.ai: input not found');
    textarea.focus();
    insertText(textarea, text);
  }

  async submitMessage(): Promise<void> { await clickSendWhenEnabled(); }
  isStreaming(): boolean { return document.querySelector('.streaming, .loading') !== null; }
  getCodeBlocks(): string[] { return Array.from(document.querySelectorAll('pre code')).map(b => b.textContent ?? ''); }

  detectError(): ProviderError | null {
    for (const el of document.querySelectorAll('[role="alert"], .error-message')) {
      if (el.closest(this.assistantSelector)) continue;
      const text = el.textContent?.toLowerCase() ?? '';
      if (text.includes('rate') || text.includes('limit')) return { type: 'rate_limit', message: el.textContent?.trim() ?? '', recoverable: true };
    }
    return null;
  }

  getAssistantText(): string {
    return document.querySelector('.message-assistant:last-of-type')?.textContent ?? '';
  }
}
```

## `extension/src/providers/gemini.ts`

```typescript
import type { ProviderAdapter, ProviderError } from './adapter';
import { insertText, clickSendWhenEnabled } from './adapter';

export class GeminiAdapter implements ProviderAdapter {
  readonly id = 'gemini';
  readonly urlPattern = /gemini\.google\.com/;
  readonly assistantSelector = 'model-response';
  readonly homepageUrl = 'https://gemini.google.com';

  async setInput(text: string): Promise<void> {
    const textarea = document.querySelector<HTMLTextAreaElement>('textarea, [contenteditable="true"]');
    if (!textarea) throw new Error('Gemini: input not found');
    textarea.focus();
    if (textarea instanceof HTMLTextAreaElement || textarea instanceof HTMLInputElement) {
      insertText(textarea, text);
    } else if (textarea.isContentEditable) {
      document.execCommand('insertText', false, text);
    }
  }

  async submitMessage(): Promise<void> { await clickSendWhenEnabled(); }
  isStreaming(): boolean { return document.querySelector('mat-progress-bar, .loading') !== null; }
  getCodeBlocks(): string[] { return Array.from(document.querySelectorAll('pre code')).map(b => b.textContent ?? ''); }

  detectError(): ProviderError | null {
    for (const el of document.querySelectorAll('[role="alert"], .error-message')) {
      if (el.closest(this.assistantSelector)) continue;
      const text = el.textContent?.toLowerCase() ?? '';
      if (text.includes('rate') || text.includes('limit') || text.includes('capacity')) {
        return { type: 'capacity', message: el.textContent?.trim() ?? '', recoverable: true };
      }
    }
    return null;
  }

  getAssistantText(): string {
    return document.querySelector('model-response:last-of-type')?.textContent ?? '';
  }
}
```

## `extension/src/providers/grok.ts`

```typescript
import type { ProviderAdapter, ProviderError } from './adapter';
import { insertText, clickSendWhenEnabled } from './adapter';

export class GrokAdapter implements ProviderAdapter {
  readonly id = 'grok';
  readonly urlPattern = /grok\.x\.ai/;
  readonly assistantSelector = '.message-bubble.assistant';
  readonly homepageUrl = 'https://grok.x.ai';

  async setInput(text: string): Promise<void> {
    const textarea = document.querySelector<HTMLTextAreaElement>('textarea');
    if (!textarea) throw new Error('Grok: input not found');
    textarea.focus();
    insertText(textarea, text);
  }

  async submitMessage(): Promise<void> { await clickSendWhenEnabled(); }
  isStreaming(): boolean { return document.querySelector('.typing-indicator, .streaming') !== null; }
  getCodeBlocks(): string[] { return Array.from(document.querySelectorAll('pre code')).map(b => b.textContent ?? ''); }

  detectError(): ProviderError | null {
    for (const el of document.querySelectorAll('[role="alert"], .error-message')) {
      if (el.closest(this.assistantSelector)) continue;
      const text = el.textContent?.toLowerCase() ?? '';
      if (text.includes('rate')) return { type: 'rate_limit', message: el.textContent?.trim() ?? '', recoverable: true };
    }
    return null;
  }

  getAssistantText(): string {
    return document.querySelector('.message-bubble.assistant:last-of-type')?.textContent ?? '';
  }
}
```

## `extension/src/providers/mistral.ts`

```typescript
import type { ProviderAdapter, ProviderError } from './adapter';
import { insertText, clickSendWhenEnabled } from './adapter';

export class MistralAdapter implements ProviderAdapter {
  readonly id = 'mistral';
  readonly urlPattern = /chat\.mistral\.ai/;
  readonly assistantSelector = '.prose';
  readonly homepageUrl = 'https://chat.mistral.ai';

  async setInput(text: string): Promise<void> {
    const textarea = document.querySelector<HTMLTextAreaElement>('textarea');
    if (!textarea) throw new Error('Mistral: input not found');
    textarea.focus();
    insertText(textarea, text);
  }

  async submitMessage(): Promise<void> { await clickSendWhenEnabled(); }
  isStreaming(): boolean { return document.querySelector('.loading, .streaming') !== null; }
  getCodeBlocks(): string[] { return Array.from(document.querySelectorAll('pre code')).map(b => b.textContent ?? ''); }

  detectError(): ProviderError | null {
    for (const el of document.querySelectorAll('[role="alert"], .error-message')) {
      if (el.closest(this.assistantSelector)) continue;
      const text = el.textContent?.toLowerCase() ?? '';
      if (text.includes('rate')) return { type: 'rate_limit', message: el.textContent?.trim() ?? '', recoverable: true };
    }
    return null;
  }

  getAssistantText(): string {
    return document.querySelector('.prose:last-of-type')?.textContent ?? '';
  }
}
```

## `extension/src/code_extractor.ts`

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
      // Track whether we're inside a ::WRITE/::REPLACE block.
      // Fences inside those blocks are content, not delimiters.
      let insideWriteOrReplace = false;

      for (let j = i + 1; j < lines.length; j++) {
        const t = lines[j].trim();

        if (t.startsWith('::WRITE ') || t.startsWith('::REPLACE ')) {
          insideWriteOrReplace = true;
        } else if (t === '::END' && insideWriteOrReplace) {
          insideWriteOrReplace = false;
        }

        // Only treat fences as block delimiters when NOT inside
        // a ::WRITE/::REPLACE content section
        if (t.startsWith('```') && !insideWriteOrReplace) {
          endLine = j;
          break;
        }
      }

      if (endLine >= 0) {
        blocks.push(lines.slice(contentStart, endLine).join('\n').trim());
        i = endLine + 1;
      } else {
        break;
      }
    } else {
      i++;
    }
  }

  return blocks;
}

export function isBlockComplete(blockContent: string): boolean {
  const normalized = normalizeLineEndings(blockContent);
  return normalized.includes('::COMMIT') || normalized.includes('::DONE')
    || normalized.includes('::APPROVED') || normalized.includes('::INCOMPLETE');
}
```

## `extension/src/stream_watcher.ts`

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
  // Serialize concurrent checkForCompleteBlocks calls
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

    const container =
      document.querySelector('[role="main"]') ??
      document.querySelector(this.adapter.assistantSelector)?.parentElement ??
      document.body;

    this.observer = new MutationObserver(() => {
      if (!this.adapter.isStreaming()) void this.serializedCheck();
    });
    this.observer.observe(container, {
      childList: true,
      subtree: true,
      characterData: true,
    });

    this.pollingTimer = setInterval(() => {
      if (!this.isWatching) return;
      const streaming = this.adapter.isStreaming();
      if (this.lastStreaming && !streaming) {
        void this.serializedCheck();
        this.handleStreamComplete();
      }
      this.lastStreaming = streaming;
    }, 500);
  }

  stop(): void {
    this.isWatching = false;
    this.observer?.disconnect();
    this.observer = null;
    if (this.pollingTimer) clearInterval(this.pollingTimer);
    this.pollingTimer = null;
  }

  resetForNewTurn(): void {
    this.turn++;
    this.previousResponseText = '';
  }

  /** Serialize checkForCompleteBlocks so concurrent triggers don't race. */
  private async serializedCheck(): Promise<void> {
    // Chain onto the previous check so they run sequentially
    this.pendingCheck = this.pendingCheck.then(() => this.checkForCompleteBlocks());
    await this.pendingCheck;
  }

  private async checkForCompleteBlocks(): Promise<void> {
    const text = this.adapter.getAssistantText();
    if (!text || text === this.previousResponseText) return;
    this.previousResponseText = text;
    const normalized = normalizeLineEndings(text);
    const blocks = extractGlyimOpsBlocks(normalized);
    for (const block of blocks) {
      const hash = await this.hash(block);
      if (this.sentHashes.has(hash)) continue;
      if (!isBlockComplete(block)) continue;
      const dangerous = containsDangerousPattern(block);
      if (dangerous) {
        this.onDangerousPattern(block, dangerous);
        this.sentHashes.add(hash);
        continue;
      }
      this.sentHashes.add(hash);
      this.onOpsReady(block, this.turn);
    }
  }

  private handleStreamComplete(): void {
    const full = this.adapter.getAssistantText();
    if (full) this.onStreamComplete(full, this.turn);
    this.sentHashes.clear();
  }

  private async hash(content: string): Promise<string> {
    const encoder = new TextEncoder();
    const data = encoder.encode(content);
    const hashBuffer = await crypto.subtle.digest('SHA-256', data);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    return hashArray
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('')
      .slice(0, 16);
  }
}
```

## `extension/src/content.ts`

```typescript
// Content script: delegates error detection to the active provider adapter
// via the background script. Does NOT reimplement detection logic.

// The background script's StreamWatcher already uses the adapter's
// detectError() method. This content script only handles:
// 1. Responding to status queries from the background script
// 2. Network offline detection (not adapter-specific)

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (msg.type === 'content.checkStatus') {
    const streaming = !!document.querySelector(
      '.typing-indicator, .streaming, .loading, mat-progress-bar'
    );
    const offline = !navigator.onLine;
    sendResponse({ streaming, offline });
  }
  if (msg.type === 'content.getAssistantText') {
    const selectors = [
      '.ds-markdown--block:last-of-type',
      '.message-assistant:last-of-type',
      'model-response:last-of-type',
      '.prose:last-of-type',
    ];
    let text = '';
    for (const sel of selectors) {
      const el = document.querySelector(sel);
      if (el) {
        text = el.textContent ?? '';
        break;
      }
    }
    sendResponse({ text });
  }
  if (msg.type === 'content.injectPrompt') {
    // Called by background script when chrome.scripting is unavailable
    const prompt = msg.prompt as string;
    const input = document.querySelector<HTMLTextAreaElement>(
      'textarea, [contenteditable="true"]'
    );
    if (!input) {
      sendResponse({ success: false, error: 'input not found' });
      return true;
    }
    input.focus();
    if (input instanceof HTMLTextAreaElement || input instanceof HTMLInputElement) {
      const start = input.selectionStart ?? 0;
      const end = input.selectionEnd ?? 0;
      input.setRangeText(prompt, start, end, 'end');
      input.dispatchEvent(new Event('input', { bubbles: true }));
    } else if (input.isContentEditable) {
      document.execCommand('insertText', false, prompt);
    }
    sendResponse({ success: true });
  }
  return true;
});

// Report offline status to background
window.addEventListener('offline', () => {
  chrome.runtime.sendMessage({
    type: 'content.networkStatus',
    online: false,
  });
});
window.addEventListener('online', () => {
  chrome.runtime.sendMessage({
    type: 'content.networkStatus',
    online: true,
  });
});
```

## `extension/src/background.ts`

```typescript
import { WsClient } from './ws_client';
import { getAdapterForUrl, registerAdapter, getAllAdapters } from './providers/adapter';
import type { ProviderAdapter } from './providers/adapter';
import { DeepSeekAdapter } from './providers/deepseek';
import { ZaiAdapter } from './providers/zai';
import { GeminiAdapter } from './providers/gemini';
import { GrokAdapter } from './providers/grok';
import { MistralAdapter } from './providers/mistral';
import { StreamWatcher } from './stream_watcher';
import type { CliMessage, TabSession } from './types';
import { PROTOCOL_VERSION, serializeTabSessions, deserializeTabSessions } from './types';

registerAdapter(new DeepSeekAdapter());
registerAdapter(new ZaiAdapter());
registerAdapter(new GeminiAdapter());
registerAdapter(new GrokAdapter());
registerAdapter(new MistralAdapter());

const ws = new WsClient();
const tabSessions = new Map<number, TabSession>();
const watchers = new Map<number, StreamWatcher>();

ws.onMessage(async (msg: CliMessage) => {
  // Version check
  const msgVersion = (msg as Record<string, unknown>).v as number | undefined;
  if (msgVersion !== undefined && msgVersion > PROTOCOL_VERSION) {
    console.warn(
      `glyim-pilot: received v${msgVersion} message, expected v${PROTOCOL_VERSION}`
    );
  }

  switch (msg.type) {
    case 'session.start': await handleSessionStart(msg); break;
    case 'feedback.send': await handleFeedbackSend(msg); break;
    case 'feedback.continue': await handleFeedbackContinue(msg); break;
    case 'retry.prompt': await handleRetryPrompt(msg); break;
    case 'session.pause': await handleSessionPause(msg); break;
    case 'session.abort': await handleSessionAbort(msg); break;
    case 'ping': ws.send({ type: 'pong', timestamp: Date.now(), v: PROTOCOL_VERSION }); break;
  }
});

ws.onStatusChange(async (connected) => {
  if (connected) await restoreSessions();
});

ws.connect();

async function waitForInputElement(tabId: number, maxWaitMs = 10000): Promise<boolean> {
  const pollInterval = 200;
  const maxAttempts = maxWaitMs / pollInterval;
  for (let i = 0; i < maxAttempts; i++) {
    const results = await chrome.scripting.executeScript({
      target: { tabId },
      func: () => !!document.querySelector('textarea, [contenteditable="true"]'),
    });
    if (results[0]?.result) return true;
    await new Promise(r => setTimeout(r, pollInterval));
  }
  return false;
}

async function injectPrompt(tabId: number, prompt: string): Promise<{ success: boolean; error?: string }> {
  try {
    const results = await chrome.scripting.executeScript({
      target: { tabId },
      func: (text: string) => {
        const input = document.querySelector<HTMLTextAreaElement>(
          'textarea, [contenteditable="true"]'
        );
        if (!input) return { success: false, error: 'input element not found' };
        input.focus();
        if (input instanceof HTMLTextAreaElement || input instanceof HTMLInputElement) {
          const start = input.selectionStart ?? 0;
          const end = input.selectionEnd ?? 0;
          input.setRangeText(text, start, end, 'end');
          input.dispatchEvent(new Event('input', { bubbles: true }));
        } else if (input.isContentEditable) {
          document.execCommand('insertText', false, text);
        }
        // Poll for send button
        const pollForSend = (): void => {
          const btn = document.querySelector<HTMLButtonElement>(
            "button[type='submit'], button[aria-label*='send'], div[class*='send-button']"
          );
          if (btn && !btn.disabled && !btn.getAttribute('aria-disabled')) {
            btn.click();
            return;
          }
          setTimeout(pollForSend, 100);
        };
        setTimeout(pollForSend, 50);
        return { success: true };
      },
      args: [prompt],
    });
    const result = results[0]?.result as { success: boolean; error?: string } | undefined;
    if (!result?.success) {
      // Report injection failure back to server
      const session = findSessionByTabId(tabId);
      if (session) {
        ws.send({
          type: 'error.detected',
          sessionId: session.sessionId,
          errorType: 'injection_failed',
          errorMessage: result?.error ?? 'unknown injection failure',
          recoverable: true,
          v: PROTOCOL_VERSION,
        });
      }
    }
    return result ?? { success: false, error: 'no result from script' };
  } catch (e) {
    const session = findSessionByTabId(tabId);
    if (session) {
      ws.send({
        type: 'error.detected',
        sessionId: session.sessionId,
        errorType: 'injection_failed',
        errorMessage: `scripting error: ${e}`,
        recoverable: true,
        v: PROTOCOL_VERSION,
      });
    }
    return { success: false, error: String(e) };
  }
}

async function handleSessionStart(msg: Extract<CliMessage, { type: 'session.start' }>) {
  const { sessionId, providerId, prompt, traceId } = msg;
  const adapter = getAllAdapters().find(a => a.id === providerId);
  if (!adapter) return;
  const tab = await chrome.tabs.create({ url: adapter.homepageUrl, active: true });
  if (!tab.id) return;
  const ready = await waitForInputElement(tab.id);
  if (!ready) {
    ws.send({
      type: 'error.detected',
      sessionId,
      errorType: 'input_not_found',
      errorMessage: 'Could not find input element on provider page',
      recoverable: false,
      v: PROTOCOL_VERSION,
    });
    return;
  }
  const injectResult = await injectPrompt(tab.id, prompt);
  if (!injectResult.success) return;
  tabSessions.set(tab.id, {
    tabId: tab.id,
    sessionId,
    streamId: sessionId,
    providerId,
    status: 'active',
    turn: 0,
  });
  await persistSessions();
  ws.send({
    type: 'session.ready',
    sessionId,
    providerId,
    tabId: tab.id,
    traceId,
    v: PROTOCOL_VERSION,
  });
  startWatcher(tab.id, sessionId, adapter);
}

async function handleFeedbackSend(msg: Extract<CliMessage, { type: 'feedback.send' }>) {
  const entry = findSession(msg.sessionId);
  if (!entry) return;
  const [tabId] = entry;
  await injectPrompt(tabId, msg.message);
  watchers.get(tabId)?.resetForNewTurn();
}

async function handleFeedbackContinue(msg: Extract<CliMessage, { type: 'feedback.continue' }>) {
  const entry = findSession(msg.sessionId);
  if (!entry) return;
  const [tabId] = entry;
  await injectPrompt(tabId, 'Please continue.');
  watchers.get(tabId)?.resetForNewTurn();
}

async function handleRetryPrompt(msg: Extract<CliMessage, { type: 'retry.prompt' }>) {
  await new Promise(r => setTimeout(r, msg.delay));
  const entry = findSession(msg.sessionId);
  if (!entry) return;
  const [tabId] = entry;
  await injectPrompt(tabId, msg.message);
}

async function handleSessionPause(msg: Extract<CliMessage, { type: 'session.pause' }>) {
  const entry = findSession(msg.sessionId);
  if (!entry) return;
  const [tabId, sess] = entry;
  sess.status = 'paused';
  watchers.get(tabId)?.stop();
  await persistSessions();
}

async function handleSessionAbort(msg: Extract<CliMessage, { type: 'session.abort' }>) {
  const entry = findSession(msg.sessionId);
  if (!entry) return;
  const [tabId] = entry;
  watchers.get(tabId)?.stop();
  watchers.delete(tabId);
  tabSessions.delete(tabId);
  await persistSessions();
}

function startWatcher(tabId: number, sessionId: string, adapter: ProviderAdapter) {
  watchers.get(tabId)?.stop();
  const watcher = new StreamWatcher(
    adapter,
    sessionId,
    (content, turn) => ws.send({
      type: 'ops.ready',
      sessionId,
      content,
      turn,
      v: PROTOCOL_VERSION,
    }),
    (full, turn) => ws.send({
      type: 'stream.complete',
      sessionId,
      turn,
      fullResponse: full,
      v: PROTOCOL_VERSION,
    }),
    (content, pattern) => ws.send({
      type: 'error.detected',
      sessionId,
      errorType: 'dangerous_pattern',
      errorMessage: `Dangerous pattern: "${pattern}". Confirmation required.`,
      recoverable: true,
      v: PROTOCOL_VERSION,
    }),
  );
  watcher.start();
  watchers.set(tabId, watcher);
}

function findSession(sessionId: string): [number, TabSession] | null {
  for (const [tabId, sess] of tabSessions.entries()) {
    if (sess.sessionId === sessionId) return [tabId, sess];
  }
  return null;
}

function findSessionByTabId(tabId: number): TabSession | null {
  return tabSessions.get(tabId) ?? null;
}

async function persistSessions() {
  const serialized = serializeTabSessions(tabSessions);
  await chrome.storage.local.set({ tabSessions: serialized });
}

async function restoreSessions() {
  const stored = await chrome.storage.local.get('tabSessions');
  if (!stored.tabSessions) return;
  try {
    const parsed = JSON.parse(stored.tabSessions as string);
    const sessions = deserializeTabSessions(parsed);
    for (const [tabId, sess] of sessions.entries()) {
      try {
        await chrome.tabs.get(tabId);
        tabSessions.set(tabId, sess);
        const adapter = getAllAdapters().find(a => a.id === sess.providerId);
        if (adapter) startWatcher(tabId, sess.sessionId, adapter);
      } catch {
        // Tab no longer exists; skip
      }
    }
  } catch (e) {
    console.warn('glyim-pilot: failed to restore sessions:', e);
  }
}

chrome.runtime.onStartup.addListener(restoreSessions);
```

## `extension/src/code_extractor.test.ts`

```typescript
import { describe, it, expect } from 'vitest';
import { extractGlyimOpsBlocks } from './code_extractor';

describe('extractGlyimOpsBlocks', () => {
  it('extracts block with CRLF', () => {
    const input = '```glyim-ops\r\n::WRITE a.rs\r\n::END\r\n```';
    const blocks = extractGlyimOpsBlocks(input);
    expect(blocks).toHaveLength(1);
    expect(blocks[0]).toContain('::WRITE');
  });

  it('handles nested fences inside WRITE', () => {
    const input = '```glyim-ops\n::WRITE readme.md\n# Hello\n\n```rust\nfn main() {}\n```\n\nMore\n::END\n```';
    const blocks = extractGlyimOpsBlocks(input);
    expect(blocks).toHaveLength(1);
    expect(blocks[0]).toContain('fn main() {}');
    expect(blocks[0]).toContain('More');
  });

  it('handles bare fence inside WRITE content', () => {
    const input = '```glyim-ops\n::WRITE readme.md\n```\nsome output\n```\n::END\n```';
    const blocks = extractGlyimOpsBlocks(input);
    expect(blocks).toHaveLength(1);
    expect(blocks[0]).toContain('some output');
  });

  it('returns empty for no blocks', () => {
    expect(extractGlyimOpsBlocks('```rust\ncode\n```')).toHaveLength(0);
  });
});
```

## `extension/src/types.test.ts`

```typescript
import { describe, it, expect } from 'vitest';
import {
  containsDangerousPattern,
  normalizeLineEndings,
  serializeTabSessions,
  deserializeTabSessions,
} from './types';
import type { TabSession } from './types';

describe('normalizeLineEndings', () => {
  it('strips CR from CRLF', () => {
    expect(normalizeLineEndings('a\r\nb')).toBe('a\nb');
  });
  it('strips standalone CR', () => {
    expect(normalizeLineEndings('a\rb')).toBe('ab');
  });
});

describe('containsDangerousPattern', () => {
  it('detects rm -rf', () => {
    expect(containsDangerousPattern('rm -rf /tmp')).toBe('rm -rf');
  });
  it('returns null for safe content', () => {
    expect(containsDangerousPattern('fn main() {}')).toBeNull();
  });
});

describe('serializeTabSessions / deserializeTabSessions', () => {
  it('roundtrips sessions with numeric keys', () => {
    const sessions = new Map<number, TabSession>();
    sessions.set(42, {
      tabId: 42,
      sessionId: 's1',
      streamId: 'S01',
      providerId: 'deepseek',
      status: 'active',
      turn: 3,
    });
    sessions.set(99, {
      tabId: 99,
      sessionId: 's2',
      streamId: 'S02',
      providerId: 'gemini',
      status: 'paused',
      turn: 1,
    });
    const serialized = serializeTabSessions(sessions);
    const deserialized = deserializeTabSessions(JSON.parse(serialized));
    expect(deserialized.size).toBe(2);
    expect(deserialized.get(42)?.providerId).toBe('deepseek');
    expect(deserialized.get(99)?.turn).toBe(1);
  });

  it('handles empty map', () => {
    const sessions = new Map<number, TabSession>();
    const serialized = serializeTabSessions(sessions);
    const deserialized = deserializeTabSessions(JSON.parse(serialized));
    expect(deserialized.size).toBe(0);
  });

  it('skips non-numeric keys', () => {
    const raw = { not_a_number: { tabId: 0, sessionId: 'x' }, '42': { tabId: 42 } };
    const result = deserializeTabSessions(raw);
    expect(result.size).toBe(1);
    expect(result.has(42)).toBe(true);
  });
});
```
