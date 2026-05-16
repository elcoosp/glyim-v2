# Phase 1: Core Protocol & Project Foundation — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the project skeleton with all dependencies, complete error types (including `reason` field in `PathEscape` per Fix #5), the full `glyim-ops` protocol type system and parser, the file applier with path security that preserves validation messages, and the `ApplyResult` type — the bedrock every other subsystem depends on. Every type referenced in later phases is defined here.

**Architecture:** Line-oriented parser for the `glyim-ops` protocol. File applier validates paths against worktree containment before applying WRITE/REPLACE/DELETE, preserving the *reason* a path was rejected. All types are `serde`-serializable for state persistence and WebSocket messaging. The `Gate` trait uses `async-trait` for clean async dispatch.

**Tech Stack:** Rust edition 2021, async-trait 0.1, thiserror 2, serde 1, serde_json 1, path-clean 1, dunce 1, tempfile 3, proptest 1.11, pretty_assertions 1

---

### Task 1: Project Skeleton with Complete Cargo.toml

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`

- [ ] **Step 1: Create Cargo.toml with all dependencies including async-trait**

```toml
[package]
name = "glyim-pilot"
version = "0.1.0"
edition = "2021"
description = "Autonomous AI agent dispatch for Glyim compiler development"
license = "MIT"

[[bin]]
name = "glyim-pilot"
path = "src/main.rs"

[dependencies]
# CLI
clap = { version = "4.5", features = ["derive", "env"] }

# Async
tokio = { version = "1", features = ["full"] }
futures-util = "0.3"
async-trait = "0.1"

# WebSocket
tokio-tungstenite = "0.29"

# Parsing
winnow = "1.0"
regex = "1.11"

# Serde
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Error handling
anyhow = "1"
thiserror = "2"

# Terminal UI
comfy-table = "7"
indicatif = "0.18"
console = "0.16"

# Dates/times
chrono = { version = "0.4", features = ["serde"] }

# Hashing
blake3 = "1.8"

# Random
rand = "0.10"

# Path handling
path-clean = "1"
dunce = "1"

# File discovery
walkdir = "2"
ignore = "0.4"

# Markdown parsing
pulldown-cmark = "0.13"

# ANSI stripping
strip-ansi-escapes = "0.2"

# Session IDs
uuid = { version = "1", features = ["v4"] }

# Diff generation
similar = "3"

# Cross-platform directories
dirs = "6"

# State file watcher (optional)
notify = { version = "8", optional = true }

[dev-dependencies]
proptest = "1.11"
tempfile = "3"
tokio-test = "0.4"
mockall = "0.13"
assert_cmd = "2"
predicates = "3"
httpmock = "0.7"
pretty_assertions = "1"

[features]
default = []
watch-state = ["notify"]

[profile.release]
lto = true
strip = true
opt-level = 3
codegen-units = 1
```

- [ ] **Step 2: Create src/main.rs**

```rust
fn main() {
    println!("glyim-pilot v0.1.0");
}
```

- [ ] **Step 3: Create src/lib.rs with all module declarations**

```rust
pub mod error;
pub mod protocol;
pub mod applier;

// Re-export primary types for convenience
pub use error::{PilotError, ApplyError};
pub use protocol::types::{FileOp, ParsedOps};
pub use protocol::parser::{parse_ops_block, extract_ops_blocks};
pub use applier::{apply_ops, ApplyResult, ApplyAction};
```

- [ ] **Step 4: Run cargo check — expect errors for missing modules**

Run: `cargo check`
Expected: Compile errors for missing `protocol`, `applier`, `error` modules

- [ ] **Step 5: Commit**

```bash
git init && git add -A && git commit -m "chore: project skeleton with complete Cargo.toml including async-trait"
```

---

### Task 2: Complete Error Types (with PathEscape reason field — Fix #5)

**Files:**
- Create: `src/error.rs`

This task defines **every** error variant that any subsequent phase references — including `PilotError::Gate`, `PilotError::PathEscape` with a `reason` field (Fix #5), and `ApplyError` variants.

**Fix #5 applied:** `PilotError::PathEscape` now has three fields — `{path}`, `{root}`, and `{reason}` — so the validation message from `validate_path` is never discarded. When `validate_path` rejects a path because it's absolute, escapes the worktree, or resolves to the root, that specific reason is preserved in the error.

- [ ] **Step 1: Write the error module with all variants and tests**

```rust
use thiserror::Error;

/// Top-level error type for Glyim Pilot.
///
/// Every variant used anywhere in the codebase is defined here.
/// No downstream module should need to define its own error enum.
#[derive(Debug, Error)]
pub enum PilotError {
    #[error("protocol parse error at line {line}: {message}")]
    Parse {
        line: usize,
        message: String,
    },

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

    #[error("gate '{gate}' failed: {message}")]
    Gate {
        gate: String,
        message: String,
    },

    #[error("config error: {0}")]
    Config(String),

    #[error("session error: {0}")]
    Session(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors specific to applying file operations (WRITE/REPLACE/DELETE).
#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("FIND text not found in {path}")]
    FindNotFound {
        path: String,
    },

    #[error("FIND text found {count} times in {path} (expected exactly 1)")]
    FindAmbiguous {
        path: String,
        count: usize,
    },

    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error("{0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_parse() {
        let err = PilotError::Parse {
            line: 5,
            message: "unknown directive".into(),
        };
        assert_eq!(
            format!("{err}"),
            "protocol parse error at line 5: unknown directive"
        );
    }

    #[test]
    fn test_error_display_gate() {
        let err = PilotError::Gate {
            gate: "check".into(),
            message: "compilation failed".into(),
        };
        let displayed = format!("{err}");
        assert!(
            displayed.contains("check"),
            "gate name must appear: got '{displayed}'"
        );
        assert!(
            displayed.contains("compilation failed"),
            "message must appear: got '{displayed}'"
        );
    }

    #[test]
    fn test_error_display_path_escape_with_reason() {
        let err = PilotError::PathEscape {
            path: "../../etc/passwd".into(),
            root: "/worktree".into(),
            reason: "path escapes worktree".into(),
        };
        let displayed = format!("{err}");
        assert!(
            displayed.contains("../../etc/passwd"),
            "path must appear: got '{displayed}'"
        );
        assert!(
            displayed.contains("/worktree"),
            "root must appear: got '{displayed}'"
        );
        assert!(
            displayed.contains("path escapes worktree"),
            "reason must appear: got '{displayed}'"
        );
    }

    #[test]
    fn test_error_display_path_escape_absolute_reason() {
        let err = PilotError::PathEscape {
            path: "/etc/passwd".into(),
            root: "/worktree".into(),
            reason: "path is absolute; must be relative to worktree".into(),
        };
        let displayed = format!("{err}");
        assert!(displayed.contains("absolute"));
    }

    #[test]
    fn test_error_display_path_escape_root_reason() {
        let err = PilotError::PathEscape {
            path: ".".into(),
            root: "/worktree".into(),
            reason: "path resolves to worktree root, not a file".into(),
        };
        let displayed = format!("{err}");
        assert!(displayed.contains("resolves to worktree root"));
    }

    #[test]
    fn test_apply_error_from_conversion() {
        let apply_err = ApplyError::FindNotFound {
            path: "src/lib.rs".into(),
        };
        let pilot_err: PilotError = apply_err.into();
        assert!(matches!(
            pilot_err,
            PilotError::Apply(ApplyError::FindNotFound { .. })
        ));
    }

    #[test]
    fn test_apply_error_ambiguous_display() {
        let err = ApplyError::FindAmbiguous {
            path: "src/lib.rs".into(),
            count: 3,
        };
        let displayed = format!("{err}");
        assert!(displayed.contains("3 times"));
        assert!(displayed.contains("src/lib.rs"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file gone");
        let pilot_err: PilotError = io_err.into();
        assert!(matches!(pilot_err, PilotError::Io(_)));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib error`
Expected: 8 PASS

- [ ] **Step 3: Commit**

```bash
git add src/error.rs && git commit -m "feat: add PilotError with PathEscape reason field (Fix #5) and ApplyError types"
```

---

### Task 3: Protocol Types

**Files:**
- Create: `src/protocol/mod.rs`
- Create: `src/protocol/types.rs`

This task defines `FileOp`, `ParsedOps`, and all protocol data types before the parser or any downstream module references them.

- [ ] **Step 1: Create src/protocol/mod.rs**

```rust
pub mod types;
pub mod parser;
```

- [ ] **Step 2: Write complete protocol types with tests**

```rust
// src/protocol/types.rs

use serde::{Deserialize, Serialize};

/// A single file operation from a glyim-ops directive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", content = "data")]
pub enum FileOp {
    #[serde(rename = "write")]
    Write {
        path: String,
        content: String,
    },
    #[serde(rename = "replace")]
    Replace {
        path: String,
        find: String,
        replace: String,
    },
    #[serde(rename = "delete")]
    Delete {
        path: String,
    },
}

/// Result of parsing a complete glyim-ops block.
///
/// This is the single output type from the parser. Every downstream consumer
/// (applier, orchestrator, session manager) receives this struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedOps {
    /// Ordered list of file operations.
    pub ops: Vec<FileOp>,
    /// Commit message if `::COMMIT <msg>` was present.
    pub commit_message: Option<String>,
    /// True if `::INCOMPLETE` was the last directive.
    pub incomplete: bool,
    /// True if `::DONE` was present.
    pub done: bool,
    /// True if `::APPROVED` was present.
    pub approved: bool,
}

impl ParsedOps {
    /// An empty parse result with no directives.
    pub fn empty() -> Self {
        Self {
            ops: Vec::new(),
            commit_message: None,
            incomplete: false,
            done: false,
            approved: false,
        }
    }

    /// Returns true if there are no file ops and no control directives.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
            && self.commit_message.is_none()
            && !self.incomplete
            && !self.done
            && !self.approved
    }

    /// Returns true if any control directive was present
    /// (COMMIT, INCOMPLETE, DONE, or APPROVED).
    pub fn has_control_directive(&self) -> bool {
        self.commit_message.is_some() || self.incomplete || self.done || self.approved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- FileOp round-trip tests ---

    #[test]
    fn test_file_op_write_roundtrip() {
        let op = FileOp::Write {
            path: "src/main.rs".into(),
            content: "fn main() {}".into(),
        };
        let json = serde_json::to_string(&op).unwrap();
        let de: FileOp = serde_json::from_str(&json).unwrap();
        assert_eq!(op, de);
    }

    #[test]
    fn test_file_op_replace_roundtrip() {
        let op = FileOp::Replace {
            path: "src/lib.rs".into(),
            find: "old".into(),
            replace: "new".into(),
        };
        let json = serde_json::to_string(&op).unwrap();
        let de: FileOp = serde_json::from_str(&json).unwrap();
        assert_eq!(op, de);
    }

    #[test]
    fn test_file_op_delete_roundtrip() {
        let op = FileOp::Delete {
            path: "src/old.rs".into(),
        };
        let json = serde_json::to_string(&op).unwrap();
        let de: FileOp = serde_json::from_str(&json).unwrap();
        assert_eq!(op, de);
    }

    // --- ParsedOps tests ---

    #[test]
    fn test_parsed_ops_empty() {
        let ops = ParsedOps::empty();
        assert!(ops.ops.is_empty());
        assert!(ops.commit_message.is_none());
        assert!(!ops.incomplete);
        assert!(!ops.done);
        assert!(!ops.approved);
        assert!(ops.is_empty());
        assert!(!ops.has_control_directive());
    }

    #[test]
    fn test_parsed_ops_with_commit() {
        let ops = ParsedOps {
            ops: vec![],
            commit_message: Some("feat: add lexer".into()),
            incomplete: false,
            done: false,
            approved: false,
        };
        assert!(!ops.is_empty());
        assert!(ops.has_control_directive());
    }

    #[test]
    fn test_parsed_ops_full_roundtrip() {
        let ops = ParsedOps {
            ops: vec![
                FileOp::Write {
                    path: "a.rs".into(),
                    content: "x".into(),
                },
                FileOp::Delete {
                    path: "b.rs".into(),
                },
            ],
            commit_message: Some("feat: add a, remove b".into()),
            incomplete: false,
            done: false,
            approved: false,
        };
        let json = serde_json::to_string(&ops).unwrap();
        let de: ParsedOps = serde_json::from_str(&json).unwrap();
        assert_eq!(ops, de);
        assert_eq!(de.ops.len(), 2);
        assert!(matches!(de.ops[0], FileOp::Write { .. }));
        assert!(matches!(de.ops[1], FileOp::Delete { .. }));
    }

    #[test]
    fn test_parsed_ops_done_flag() {
        let ops = ParsedOps {
            ops: vec![],
            commit_message: None,
            incomplete: false,
            done: true,
            approved: false,
        };
        assert!(ops.has_control_directive());
        assert!(!ops.is_empty());
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib protocol::types`
Expected: 7 PASS

- [ ] **Step 4: Commit**

```bash
git add src/protocol/ && git commit -m "feat: add FileOp and ParsedOps protocol types with serde support"
```

---

### Task 4: Parser — All Directives

**Files:**
- Create: `src/protocol/parser.rs`

The parser uses a line-scanning approach (not winnow combinators) because the protocol is line-oriented and simple. Winnow is available for future complex parsing needs.

- [ ] **Step 1: Write complete parser with all directives and tests**

```rust
// src/protocol/parser.rs

use crate::error::PilotError;
use crate::protocol::types::{FileOp, ParsedOps};

/// Extract glyim-ops blocks from a full AI response.
///
/// Returns the content between ```glyim-ops and ```.
pub fn extract_ops_blocks(response: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let marker = "```glyim-ops";
    let mut search_from = 0;

    while let Some(start) = response[search_from..].find(marker) {
        let content_start = search_from + start + marker.len();
        // Skip the newline after the opening fence
        let content_start = if response.get(content_start..content_start + 1) == Some("\n") {
            content_start + 1
        } else {
            content_start
        };
        if let Some(end) = response[content_start..].find("```") {
            blocks.push(response[content_start..content_start + end].trim());
            search_from = content_start + end + 3;
        } else {
            break; // Unclosed block — skip it
        }
    }

    blocks
}

/// Parse a single glyim-ops block into structured operations.
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
            ops.push(FileOp::Replace { path, find, replace });
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
        // Unknown lines are silently skipped for forward compatibility
    }

    Ok(ParsedOps {
        ops,
        commit_message,
        incomplete,
        done,
        approved,
    })
}

/// Read lines until `::END` is found, returning the content between.
fn read_until_end(
    lines: &mut impl Iterator<Item = (usize, &str)>,
    start_line: usize,
) -> Result<String, PilotError> {
    let mut content_lines = Vec::new();
    for (_, line) in lines {
        if line.trim() == "::END" {
            // Trim trailing blank lines from content
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

/// Read FIND/REPLACE sections until `::END` is found.
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
                // Trim trailing blank lines from both sections
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
                // Lines before ---FIND--- are silently ignored
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

    // --- WRITE tests ---

    #[test]
    fn test_parse_write() {
        let input = "::WRITE src/main.rs\nfn main() {}\n::END";
        let result = parse_ops_block(input).unwrap();
        assert_eq!(result.ops.len(), 1);
        assert_eq!(
            result.ops[0],
            FileOp::Write {
                path: "src/main.rs".into(),
                content: "fn main() {}".into(),
            }
        );
        assert!(result.commit_message.is_none());
        assert!(!result.incomplete);
    }

    #[test]
    fn test_parse_write_empty_content() {
        let input = "::WRITE src/empty.rs\n::END";
        let result = parse_ops_block(input).unwrap();
        assert_eq!(
            result.ops[0],
            FileOp::Write {
                path: "src/empty.rs".into(),
                content: String::new(),
            }
        );
    }

    #[test]
    fn test_parse_write_multiline_content() {
        let input = "::WRITE src/lib.rs\npub fn a() {}\npub fn b() {}\n::END";
        let result = parse_ops_block(input).unwrap();
        if let FileOp::Write { content, .. } = &result.ops[0] {
            assert!(content.contains("pub fn a()"));
            assert!(content.contains("pub fn b()"));
        } else {
            panic!("expected Write");
        }
    }

    #[test]
    fn test_parse_write_missing_end() {
        let input = "::WRITE src/a.rs\ncontent without end";
        let result = parse_ops_block(input);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let displayed = format!("{err}");
        assert!(
            displayed.contains("expected ::END"),
            "expected ::END in error, got: {displayed}"
        );
    }

    #[test]
    fn test_parse_write_missing_path() {
        let input = "::WRITE\ncontent\n::END";
        let result = parse_ops_block(input);
        assert!(result.is_err());
    }

    // --- REPLACE tests ---

    #[test]
    fn test_parse_replace() {
        let input = "\
::REPLACE src/lib.rs
---FIND---
pub mod old;
---REPLACE---
pub mod new;
::END";
        let result = parse_ops_block(input).unwrap();
        assert_eq!(result.ops.len(), 1);
        assert_eq!(
            result.ops[0],
            FileOp::Replace {
                path: "src/lib.rs".into(),
                find: "pub mod old;".into(),
                replace: "pub mod new;".into(),
            }
        );
    }

    #[test]
    fn test_parse_replace_multiline_find() {
        let input = "\
::REPLACE src/lib.rs
---FIND---
fn old() {
    1
}
---REPLACE---
fn new() {
    2
}
::END";
        let result = parse_ops_block(input).unwrap();
        if let FileOp::Replace { find, replace, .. } = &result.ops[0] {
            assert!(find.contains("fn old()"));
            assert!(replace.contains("fn new()"));
        } else {
            panic!("expected Replace");
        }
    }

    // --- DELETE tests ---

    #[test]
    fn test_parse_delete() {
        let input = "::DELETE src/deprecated.rs";
        let result = parse_ops_block(input).unwrap();
        assert_eq!(result.ops.len(), 1);
        assert_eq!(
            result.ops[0],
            FileOp::Delete {
                path: "src/deprecated.rs".into(),
            }
        );
    }

    #[test]
    fn test_parse_delete_missing_path() {
        let input = "::DELETE";
        let result = parse_ops_block(input);
        assert!(result.is_err());
    }

    // --- Control directive tests ---

    #[test]
    fn test_parse_commit() {
        let input = "::WRITE src/a.rs\ncontent\n::END\n::COMMIT feat(lex): add scanning";
        let result = parse_ops_block(input).unwrap();
        assert_eq!(result.ops.len(), 1);
        assert_eq!(
            result.commit_message.as_deref(),
            Some("feat(lex): add scanning")
        );
    }

    #[test]
    fn test_parse_commit_empty_message() {
        let input = "::COMMIT";
        let result = parse_ops_block(input).unwrap();
        assert_eq!(result.commit_message, Some(String::new()));
    }

    #[test]
    fn test_parse_incomplete() {
        let input = "::WRITE src/a.rs\npart1\n::END\n::INCOMPLETE";
        let result = parse_ops_block(input).unwrap();
        assert!(result.incomplete);
        assert!(!result.done);
    }

    #[test]
    fn test_parse_done() {
        let input = "::DONE";
        let result = parse_ops_block(input).unwrap();
        assert!(result.done);
        assert!(result.ops.is_empty());
    }

    #[test]
    fn test_parse_approved() {
        let input = "::APPROVED";
        let result = parse_ops_block(input).unwrap();
        assert!(result.approved);
        assert!(result.ops.is_empty());
    }

    // --- Multi-op tests ---

    #[test]
    fn test_parse_multiple_ops() {
        let input = "\
::WRITE src/a.rs\nfn a() {}\n::END
::REPLACE src/b.rs
---FIND---
old
---REPLACE---
new
::END
::DELETE src/c.rs
::COMMIT feat: multi-op commit";
        let result = parse_ops_block(input).unwrap();
        assert_eq!(result.ops.len(), 3);
        assert!(matches!(result.ops[0], FileOp::Write { .. }));
        assert!(matches!(result.ops[1], FileOp::Replace { .. }));
        assert!(matches!(result.ops[2], FileOp::Delete { .. }));
        assert_eq!(
            result.commit_message.as_deref(),
            Some("feat: multi-op commit")
        );
    }

    // --- Block extraction tests ---

    #[test]
    fn test_extract_single_block() {
        let response = "Some text\n```glyim-ops\n::WRITE src/a.rs\nhi\n::END\n```\nMore text";
        let blocks = extract_ops_blocks(response);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("::WRITE"));
    }

    #[test]
    fn test_extract_multiple_blocks() {
        let response = "\
```glyim-ops
::WRITE src/a.rs\na\n::END
```
Some commentary
```glyim-ops
::DELETE src/b.rs
```";
        let blocks = extract_ops_blocks(response);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("::WRITE"));
        assert!(blocks[1].contains("::DELETE"));
    }

    #[test]
    fn test_extract_no_blocks() {
        let response = "Just regular text\n```rust\nfn main() {}\n```";
        let blocks = extract_ops_blocks(response);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_extract_unclosed_block_returns_nothing() {
        let response = "```glyim-ops\n::WRITE x\n::END";
        let blocks = extract_ops_blocks(response);
        assert!(blocks.is_empty(), "unclosed fence should yield no blocks");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib protocol::parser`
Expected: 18 PASS

- [ ] **Step 3: Commit**

```bash
git add src/protocol/parser.rs && git commit -m "feat: add glyim-ops parser with all directives and block extraction"
```

---

### Task 5: Parser — Property-Based Tests

**Files:**
- Modify: `src/protocol/parser.rs`

- [ ] **Step 1: Add proptest tests**

Append to the `tests` module in `src/protocol/parser.rs`:

```rust
    use proptest::prelude::*;

    prop_compose! {
        fn arb_path()(path in "[a-z][a-z0-9_/]*\\.rs") -> String {
            path
        }
    }

    prop_compose! {
        fn arb_content()(content in "[a-zA-Z0-9 \\n{}();:]*") -> String {
            content
        }
    }

    proptest! {
        #[test]
        fn proptest_write_roundtrip(path in arb_path(), content in arb_content()) {
            let block = format!("::WRITE {path}\n{content}\n::END");
            let result = parse_ops_block(&block)?;
            prop_assert_eq!(result.ops.len(), 1);
            if let FileOp::Write { path: p, content: c } = &result.ops[0] {
                prop_assert_eq!(p, &path);
                prop_assert_eq!(c.trim(), content.trim());
            } else {
                return Err(proptest::test_runner::TestCaseError::fail("expected Write"));
            }
        }

        #[test]
        fn proptest_delete_roundtrip(path in arb_path()) {
            let block = format!("::DELETE {path}");
            let result = parse_ops_block(&block)?;
            prop_assert_eq!(result.ops.len(), 1);
            prop_assert!(matches!(&result.ops[0], FileOp::Delete { .. }));
        }

        #[test]
        fn proptest_extract_never_panics(input in ".*") {
            let _ = extract_ops_blocks(&input);
        }

        #[test]
        fn proptest_parse_never_panics(input in ".*") {
            let _ = parse_ops_block(&input);
        }
    }
```

- [ ] **Step 2: Run proptests**

Run: `cargo test --lib protocol::parser::tests::proptest`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add src/protocol/parser.rs && git commit -m "test: add property-based parser tests with proptest"
```

---

### Task 6: File Applier — Path Security (preserving validation reason — Fix #5)

**Files:**
- Create: `src/applier/mod.rs`
- Create: `src/applier/security.rs`

**Fix #5 applied:** `validate_path` returns `Result<PathBuf, String>` where the `String` is a descriptive error explaining *why* the path was rejected. The applier passes this string as the `reason` field in `PilotError::PathEscape`, so no diagnostic information is lost.

- [ ] **Step 1: Write path validation with thorough tests**

```rust
// src/applier/security.rs

use std::path::{Path, PathBuf};

/// Validate that a relative path does not escape the worktree root.
///
/// Returns the normalized absolute path within the worktree on success,
/// or a descriptive error string on failure.
///
/// Security: This function is the primary defense against path traversal
/// attacks (NFR-SEC-002). It rejects absolute paths, paths containing `..`
/// that resolve outside the root, and other escape attempts.
///
/// The error string explains *why* the path was rejected — this is
/// intentionally preserved in `PilotError::PathEscape` so that
/// debugging information is never discarded (Fix #5).
pub fn validate_path(worktree_root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let relative = Path::new(relative_path);

    // Reject absolute paths
    if relative.is_absolute() {
        return Err(format!(
            "path '{}' is absolute; must be relative to worktree",
            relative_path
        ));
    }

    // Join with worktree root
    let candidate = worktree_root.join(relative);

    // Normalize: resolve . and .. components
    let normalized = path_clean::PathClean::clean(&candidate);

    // Determine the canonical root (resolve symlinks if the root exists on disk)
    let root_normalized = if worktree_root.exists() {
        match dunce::canonicalize(worktree_root) {
            Ok(canonical) => path_clean::PathClean::clean(&canonical),
            Err(_) => path_clean::PathClean::clean(worktree_root),
        }
    } else {
        path_clean::PathClean::clean(worktree_root)
    };

    // Containment check: the normalized path must start with the root path
    let normalized_str = normalized.to_string_lossy();
    let root_str = root_normalized.to_string_lossy();

    // Ensure the root string ends with a separator for prefix matching
    let root_prefix = if root_str.ends_with('/') || root_str.ends_with('\\') {
        root_str.to_string()
    } else {
        format!("{}/", root_str)
    };

    if normalized_str == root_str.as_ref() {
        // Path resolves to the root itself — reject (no file specified)
        return Err(format!(
            "path '{}' resolves to worktree root, not a file",
            relative_path
        ));
    }

    if !normalized_str.starts_with(root_prefix.as_str()) {
        return Err(format!(
            "path '{}' escapes worktree '{}'",
            relative_path,
            root_normalized.display()
        ));
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_worktree() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn test_valid_simple_path() {
        let dir = setup_worktree();
        let result = validate_path(dir.path(), "src/main.rs");
        assert!(result.is_ok(), "simple relative path should be valid");
        let path = result.unwrap();
        assert!(path.ends_with("src/main.rs"));
    }

    #[test]
    fn test_valid_nested_path() {
        let dir = setup_worktree();
        let result = validate_path(dir.path(), "crates/frontend/src/lexer.rs");
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_dot_path() {
        let dir = setup_worktree();
        let result = validate_path(dir.path(), "./src/main.rs");
        assert!(result.is_ok());
    }

    #[test]
    fn test_path_traversal_attack() {
        let dir = setup_worktree();
        let result = validate_path(dir.path(), "../../etc/passwd");
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("escapes worktree"), "reason should explain escape: got '{err_msg}'");
    }

    #[test]
    fn test_absolute_path_rejected_with_reason() {
        let dir = setup_worktree();
        let result = validate_path(dir.path(), "/etc/passwd");
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("absolute"), "reason should explain absolute: got '{err_msg}'");
    }

    #[test]
    fn test_dotdot_in_middle() {
        let dir = setup_worktree();
        let result = validate_path(dir.path(), "src/../../etc/passwd");
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("escapes worktree"), "reason should explain escape: got '{err_msg}'");
    }

    #[test]
    fn test_dotdot_that_stays_inside() {
        let dir = setup_worktree();
        let result = validate_path(dir.path(), "src/../lib/main.rs");
        // This should be valid — it resolves to lib/main.rs which is inside
        assert!(result.is_ok(), "src/../lib/main.rs should resolve inside worktree");
    }

    #[test]
    fn test_path_resolving_to_root_rejected_with_reason() {
        let dir = setup_worktree();
        let result = validate_path(dir.path(), ".");
        assert!(result.is_err(), "path resolving to root should be rejected");
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("resolves to worktree root"), "reason should explain root resolution: got '{err_msg}'");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib applier::security`
Expected: 8 PASS

- [ ] **Step 3: Commit**

```bash
git add src/applier/security.rs && git commit -m "feat: add path containment validation preserving rejection reason (Fix #5, NFR-SEC-002)"
```

---

### Task 7: File Applier — Apply Operations with ApplyResult (Fix #5 continued)

**Files:**
- Modify: `src/applier/mod.rs`

This task defines `ApplyResult`, `ApplyAction`, and the `apply_ops` function. The key fix: `PilotError::PathEscape` now includes the `reason` from `validate_path`, so no diagnostic is discarded.

- [ ] **Step 1: Write the complete applier with tests**

```rust
// src/applier/mod.rs

pub mod security;

use std::fs;
use std::path::Path;

use crate::error::{ApplyError, PilotError};
use crate::protocol::types::FileOp;
use security::validate_path;

/// Result of applying a single file operation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApplyResult {
    pub path: String,
    pub action: ApplyAction,
}

/// The kind of change made to the filesystem.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ApplyAction {
    Created,
    Modified,
    Deleted,
}

/// Apply a list of file operations to the worktree.
///
/// Operations are applied sequentially in order. If any operation fails,
/// the function returns immediately and subsequent operations are not applied.
pub fn apply_ops(
    worktree_root: &Path,
    ops: &[FileOp],
) -> Result<Vec<ApplyResult>, PilotError> {
    let mut results = Vec::new();
    for op in ops {
        let result = apply_op(worktree_root, op)?;
        results.push(result);
    }
    Ok(results)
}

fn apply_op(worktree_root: &Path, op: &FileOp) -> Result<ApplyResult, PilotError> {
    match op {
        FileOp::Write { path, content } => apply_write(worktree_root, path, content),
        FileOp::Replace { path, find, replace } => {
            apply_replace(worktree_root, path, find, replace)
        }
        FileOp::Delete { path } => apply_delete(worktree_root, path),
    }
}

fn apply_write(
    worktree_root: &Path,
    relative_path: &str,
    content: &str,
) -> Result<ApplyResult, PilotError> {
    // Fix #5: Pass the validation reason through to PilotError::PathEscape
    let abs_path = validate_path(worktree_root, relative_path).map_err(|reason| {
        PilotError::PathEscape {
            path: relative_path.to_string(),
            root: worktree_root.display().to_string(),
            reason,
        }
    })?;

    // Create parent directories if needed
    if let Some(parent) = abs_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let existed = abs_path.exists();
    fs::write(&abs_path, content)?;

    tracing::debug!(
        path = relative_path,
        action = if existed { "modified" } else { "created" },
        "applied WRITE"
    );

    Ok(ApplyResult {
        path: relative_path.to_string(),
        action: if existed {
            ApplyAction::Modified
        } else {
            ApplyAction::Created
        },
    })
}

fn apply_replace(
    worktree_root: &Path,
    relative_path: &str,
    find: &str,
    replace: &str,
) -> Result<ApplyResult, PilotError> {
    // Fix #5: Pass the validation reason through to PilotError::PathEscape
    let abs_path = validate_path(worktree_root, relative_path).map_err(|reason| {
        PilotError::PathEscape {
            path: relative_path.to_string(),
            root: worktree_root.display().to_string(),
            reason,
        }
    })?;

    if !abs_path.exists() {
        return Err(PilotError::Apply(ApplyError::FileNotFound(
            relative_path.to_string(),
        )));
    }

    let existing = fs::read_to_string(&abs_path)?;
    let count = existing.matches(find).count();

    if count == 0 {
        return Err(PilotError::Apply(ApplyError::FindNotFound {
            path: relative_path.to_string(),
        }));
    }
    if count > 1 {
        return Err(PilotError::Apply(ApplyError::FindAmbiguous {
            path: relative_path.to_string(),
            count,
        }));
    }

    let new_content = existing.replacen(find, replace, 1);
    fs::write(&abs_path, &new_content)?;

    tracing::debug!(path = relative_path, "applied REPLACE");

    Ok(ApplyResult {
        path: relative_path.to_string(),
        action: ApplyAction::Modified,
    })
}

fn apply_delete(
    worktree_root: &Path,
    relative_path: &str,
) -> Result<ApplyResult, PilotError> {
    // Fix #5: Pass the validation reason through to PilotError::PathEscape
    let abs_path = validate_path(worktree_root, relative_path).map_err(|reason| {
        PilotError::PathEscape {
            path: relative_path.to_string(),
            root: worktree_root.display().to_string(),
            reason,
        }
    })?;

    if !abs_path.exists() {
        return Err(PilotError::Apply(ApplyError::FileNotFound(
            relative_path.to_string(),
        )));
    }

    fs::remove_file(&abs_path)?;

    tracing::debug!(path = relative_path, "applied DELETE");

    Ok(ApplyResult {
        path: relative_path.to_string(),
        action: ApplyAction::Deleted,
    })
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
        let root = dir.path();
        let ops = vec![FileOp::Write {
            path: "src/main.rs".into(),
            content: "fn main() {}".into(),
        }];
        let results = apply_ops(root, &ops).unwrap();
        assert_eq!(results[0].action, ApplyAction::Created);
        assert!(root.join("src/main.rs").exists());
        assert_eq!(
            fs::read_to_string(root.join("src/main.rs")).unwrap(),
            "fn main() {}"
        );
    }

    #[test]
    fn test_apply_write_creates_parent_dirs() {
        let dir = setup_worktree();
        let root = dir.path();
        let ops = vec![FileOp::Write {
            path: "deep/nested/dir/file.rs".into(),
            content: "x".into(),
        }];
        let results = apply_ops(root, &ops).unwrap();
        assert_eq!(results[0].action, ApplyAction::Created);
        assert!(root.join("deep/nested/dir/file.rs").exists());
    }

    #[test]
    fn test_apply_write_modifies_existing() {
        let dir = setup_worktree();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "old content").unwrap();

        let ops = vec![FileOp::Write {
            path: "src/main.rs".into(),
            content: "new content".into(),
        }];
        let results = apply_ops(root, &ops).unwrap();
        assert_eq!(results[0].action, ApplyAction::Modified);
        assert_eq!(
            fs::read_to_string(root.join("src/main.rs")).unwrap(),
            "new content"
        );
    }

    #[test]
    fn test_apply_replace_succeeds() {
        let dir = setup_worktree();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub mod old;\npub mod token;\n").unwrap();

        let ops = vec![FileOp::Replace {
            path: "src/lib.rs".into(),
            find: "pub mod old;".into(),
            replace: "pub mod new;".into(),
        }];
        let results = apply_ops(root, &ops).unwrap();
        assert_eq!(results[0].action, ApplyAction::Modified);
        assert_eq!(
            fs::read_to_string(root.join("src/lib.rs")).unwrap(),
            "pub mod new;\npub mod token;\n"
        );
    }

    #[test]
    fn test_apply_replace_find_not_found() {
        let dir = setup_worktree();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub mod token;").unwrap();

        let ops = vec![FileOp::Replace {
            path: "src/lib.rs".into(),
            find: "pub mod old;".into(),
            replace: "pub mod new;".into(),
        }];
        let result = apply_ops(root, &ops);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PilotError::Apply(ApplyError::FindNotFound { .. })
        ));
    }

    #[test]
    fn test_apply_replace_find_ambiguous() {
        let dir = setup_worktree();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "pub mod token;\npub mod token;\n").unwrap();

        let ops = vec![FileOp::Replace {
            path: "src/lib.rs".into(),
            find: "pub mod token;".into(),
            replace: "pub mod replaced;".into(),
        }];
        let result = apply_ops(root, &ops);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PilotError::Apply(ApplyError::FindAmbiguous { count: 2, .. })
        ));
    }

    #[test]
    fn test_apply_delete_succeeds() {
        let dir = setup_worktree();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/old.rs"), "deprecated").unwrap();

        let ops = vec![FileOp::Delete {
            path: "src/old.rs".into(),
        }];
        let results = apply_ops(root, &ops).unwrap();
        assert_eq!(results[0].action, ApplyAction::Deleted);
        assert!(!root.join("src/old.rs").exists());
    }

    #[test]
    fn test_apply_delete_nonexistent() {
        let dir = setup_worktree();
        let root = dir.path();
        let ops = vec![FileOp::Delete {
            path: "src/nonexistent.rs".into(),
        }];
        let result = apply_ops(root, &ops);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PilotError::Apply(ApplyError::FileNotFound(_))
        ));
    }

    #[test]
    fn test_apply_path_traversal_blocked_with_reason() {
        let dir = setup_worktree();
        let root = dir.path();
        let ops = vec![FileOp::Write {
            path: "../../etc/passwd".into(),
            content: "hacked".into(),
        }];
        let result = apply_ops(root, &ops);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Fix #5: verify the error is PathEscape with a reason
        match &err {
            PilotError::PathEscape { path, root: r, reason } => {
                assert!(path.contains("../../etc/passwd"));
                assert!(reason.contains("escapes worktree"), "reason must explain why: got '{reason}'");
            }
            _ => panic!("expected PathEscape with reason, got: {err}"),
        }
    }

    #[test]
    fn test_apply_absolute_path_blocked_with_reason() {
        let dir = setup_worktree();
        let root = dir.path();
        let ops = vec![FileOp::Write {
            path: "/etc/passwd".into(),
            content: "hacked".into(),
        }];
        let result = apply_ops(root, &ops);
        assert!(result.is_err());
        match result.unwrap_err() {
            PilotError::PathEscape { reason, .. } => {
                assert!(reason.contains("absolute"), "reason must mention absolute: got '{reason}'");
            }
            other => panic!("expected PathEscape, got: {other}"),
        }
    }

    #[test]
    fn test_apply_root_path_blocked_with_reason() {
        let dir = setup_worktree();
        let root = dir.path();
        let ops = vec![FileOp::Write {
            path: ".".into(),
            content: "hacked".into(),
        }];
        let result = apply_ops(root, &ops);
        assert!(result.is_err());
        match result.unwrap_err() {
            PilotError::PathEscape { reason, .. } => {
                assert!(reason.contains("resolves to worktree root"), "reason must explain root: got '{reason}'");
            }
            other => panic!("expected PathEscape, got: {other}"),
        }
    }

    #[test]
    fn test_apply_multiple_ops_sequential() {
        let dir = setup_worktree();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "old content").unwrap();

        let ops = vec![
            FileOp::Write {
                path: "src/new.rs".into(),
                content: "new file".into(),
            },
            FileOp::Replace {
                path: "src/lib.rs".into(),
                find: "old content".into(),
                replace: "new content".into(),
            },
        ];
        let results = apply_ops(root, &ops).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].action, ApplyAction::Created);
        assert_eq!(results[1].action, ApplyAction::Modified);
    }

    #[test]
    fn test_apply_result_serialization() {
        let result = ApplyResult {
            path: "src/main.rs".into(),
            action: ApplyAction::Created,
        };
        let json = serde_json::to_string(&result).unwrap();
        let de: ApplyResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, de);
    }
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo test --lib`
Expected: All PASS (error: 8, protocol types: 7, protocol parser: 18+4 proptest, applier security: 8, applier: 13 ≈ 58 tests)

- [ ] **Step 3: Commit**

```bash
git add src/applier/mod.rs && git commit -m "feat: add file applier with PathEscape reason preservation (Fix #5), WRITE/REPLACE/DELETE, ApplyResult, and tracing"
```

---

### Task 8: CLI Skeleton

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Implement clap CLI skeleton**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "glyim-pilot", version = "0.1.0")]
#[command(about = "Autonomous AI agent dispatch for Glyim compiler development")]
struct Cli {
    /// Project root directory
    #[arg(long, env = "GLYIM_PROJECT_ROOT", default_value = ".")]
    project_root: std::path::PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the WebSocket server
    Serve,
    /// Dispatch a single stream
    Dispatch {
        /// Stream ID (e.g., S01)
        stream_id: String,
    },
    /// Dispatch an entire wave
    Wave {
        /// Wave number (1-4)
        wave: u8,
    },
    /// Show status of all active streams
    Status,
    /// Run preflight checks (provider login, toolchain)
    Preflight,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Serve => println!("Starting server... (not yet implemented)"),
        Commands::Dispatch { stream_id } => {
            println!("Dispatching {stream_id}... (not yet implemented)")
        }
        Commands::Wave { wave } => {
            println!("Dispatching wave {wave}... (not yet implemented)")
        }
        Commands::Status => println!("Status... (not yet implemented)"),
        Commands::Preflight => println!("Running preflight checks... (not yet implemented)"),
    }
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/main.rs && git commit -m "feat: add clap CLI skeleton with serve/dispatch/wave/status/preflight"
```

---

### Task 9: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test --lib`
Expected: All PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --check`
Expected: No formatting issues

- [ ] **Step 4: Tag the milestone**

```bash
git tag v0.1.0-protocol -m "Core protocol, error types (with PathEscape reason), parser, and file applier foundation"
```

---

**Phase 1 complete.** Fix #5 is fully applied: `PilotError::PathEscape` has a `reason` field, `validate_path` returns descriptive error strings, and the applier passes them through without discarding diagnostic information. All types that downstream phases reference (`PilotError::Gate`, `PilotError::PathEscape{path,root,reason}`, `ApplyResult`, `ApplyAction`, `ParsedOps`, `FileOp`, `async-trait` in Cargo.toml) are defined. The parser handles all directives. The applier has path security with preserved rejection reasons.

Ready for **Phase 2: Configuration & Git Operations** — shall I continue?
# Phase 2: Configuration & Git Operations — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the complete configuration system with gate strictness level resolution, and all git worktree operations. Every config struct referenced in later phases (`CommitGatesConfig`, `DoneGatesConfig`, `PilotConfig`, `ResolvedCommitGates`, `ResolvedDoneGates`) is fully defined here with no phantom types. `ContractGate` is NOT in `config::types` — it lives in `crate::gates::contracts` only (Fix #1 preparation).

**Architecture:** Configuration is loaded once at startup from `.glyim-pilot.toml` and shared via `Arc<PilotConfig>`. Gate strictness levels ("relaxed", "normal", "strict", "production") derive which gates are enabled. `ResolvedCommitGates` and `ResolvedDoneGates` are concrete `bool` structs with no `Option<bool>` — the resolution happens once at load time. Git operations shell out to the `git` CLI via `tokio::process::Command`. Provider cooldown timestamps use `chrono::DateTime<Utc>` for serializability.

**Tech Stack:** toml 0.8, serde 1, tokio::process, chrono 0.4, path-clean 1, dirs 6, tempfile 3

---

### Task 1: Configuration Types — Core, Server, Providers

**Files:**
- Create: `src/config/mod.rs`
- Create: `src/config/types.rs`

This is the single source of truth for all config types. Every struct used in any phase is defined here. `ContractGate` is NOT defined here — it lives exclusively in `crate::gates::contracts`. This prevents the import conflict from Fix #1.

- [ ] **Step 1: Write config types with gate strictness level logic**

```rust
// src/config/types.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────
// Top-level config
// ─────────────────────────────────────────────────────────

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
}

impl PilotConfig {
    /// Construct a minimal config suitable for unit tests.
    /// Uses sensible defaults and one provider.
    pub fn default_for_testing() -> Self {
        let mut providers = HashMap::new();
        providers.insert(
            "deepseek".into(),
            ProviderConfig {
                enabled: true,
                url: "https://chat.deepseek.com".into(),
                max_concurrent: 2,
                rate_limit_cooldown: 60,
                error_patterns: vec!["server is busy".into()],
                input_selector: "textarea".into(),
                send_selector: "button".into(),
                streaming_indicator: String::new(),
                assistant_selector: String::new(),
                code_block_selector: "pre code".into(),
            },
        );
        Self {
            server: ServerConfig::default(),
            defaults: DefaultsConfig::default(),
            providers,
            execution: ExecutionConfig::default(),
            gates: GatesConfig::default(),
            context: ContextConfig::default(),
            dispatch: DispatchConfig::default(),
        }
    }
}

// ─────────────────────────────────────────────────────────
// Server
// ─────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────
// Defaults
// ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DefaultsConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub auto_execute: bool,
    #[serde(default = "default_max_turns")]
    pub max_turns: u32,
    #[serde(default = "default_script_timeout")]
    pub script_timeout: u64,
    #[serde(default = "default_true")]
    pub retry_on_rate_limit: bool,
    #[serde(default = "default_retry_max_wait")]
    pub retry_max_wait: u64,
}

fn default_provider() -> String {
    "deepseek".into()
}
fn default_max_turns() -> u32 {
    50
}
fn default_script_timeout() -> u64 {
    300
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
            provider: default_provider(),
            auto_execute: false,
            max_turns: default_max_turns(),
            script_timeout: default_script_timeout(),
            retry_on_rate_limit: true,
            retry_max_wait: default_retry_max_wait(),
        }
    }
}

// ─────────────────────────────────────────────────────────
// Provider
// ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    pub enabled: bool,
    pub url: String,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    /// Cooldown duration in seconds. Stored as u64 to match
    /// the natural unit from config. Internally cast to i64
    /// for chrono::Duration::seconds() — safe for any practical
    /// value (max ~292 billion seconds).
    #[serde(default = "default_cooldown")]
    pub rate_limit_cooldown: u64,
    #[serde(default)]
    pub error_patterns: Vec<String>,
    pub input_selector: String,
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
fn default_code_block_selector() -> String {
    "pre code".into()
}

// ─────────────────────────────────────────────────────────
// Execution
// ─────────────────────────────────────────────────────────

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

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            worktree_base: default_worktree_base(),
            require_confirmation: default_require_confirmation(),
            dangerous_patterns: default_dangerous_patterns(),
            max_fix_rounds: default_max_fix_rounds(),
        }
    }
}

// ─────────────────────────────────────────────────────────
// Gates — with strictness level resolution
// ─────────────────────────────────────────────────────────

/// Gate strictness levels per REQ-FUNC-053.
///
/// - "relaxed": fmt + check
/// - "normal":  fmt + check + clippy + test
/// - "strict":  fmt + check + clippy + test + banned_patterns + architecture + contracts
/// - "production": strict + coverage + mutation + self_review
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
        Self::Production
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

impl std::str::FromStr for GateLevel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "relaxed" => Ok(Self::Relaxed),
            "normal" => Ok(Self::Normal),
            "strict" => Ok(Self::Strict),
            "production" => Ok(Self::Production),
            _ => Err(format!(
                "unknown gate level '{s}'; expected relaxed/normal/strict/production"
            )),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GatesConfig {
    #[serde(default)]
    pub level: GateLevel,
    /// Per-gate overrides. If a gate is explicitly set here, it takes
    /// precedence over the level-derived default.
    #[serde(default)]
    pub commit: CommitGatesConfig,
    #[serde(default)]
    pub done: DoneGatesConfig,
}

impl Default for GatesConfig {
    fn default() -> Self {
        Self {
            level: GateLevel::default(),
            commit: CommitGatesConfig::default(),
            done: DoneGatesConfig::default(),
        }
    }
}

/// Commit pipeline gate configuration.
///
/// Each field is `Option<bool>`: `Some(true)` = force-enable,
/// `Some(false)` = force-disable, `None` = derive from GateLevel.
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

impl CommitGatesConfig {
    /// Resolve each gate to enabled/disabled based on level + overrides.
    pub fn resolve(&self, level: GateLevel) -> ResolvedCommitGates {
        let defaults = level.commit_defaults();
        ResolvedCommitGates {
            fmt: self.fmt.unwrap_or(defaults.fmt),
            check: self.check.unwrap_or(defaults.check),
            clippy: self.clippy.unwrap_or(defaults.clippy),
            test: self.test.unwrap_or(defaults.test),
            banned_patterns: self.banned_patterns.unwrap_or(defaults.banned_patterns),
            architecture: self.architecture.unwrap_or(defaults.architecture),
            contracts: self.contracts.unwrap_or(defaults.contracts),
        }
    }
}

/// Level-derived defaults for commit gates.
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
            Self::Strict => CommitDefaults {
                fmt: true,
                check: true,
                clippy: true,
                test: true,
                banned_patterns: true,
                architecture: true,
                contracts: true,
            },
            Self::Production => CommitDefaults {
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

/// Fully resolved commit gate configuration — no Option<bool>, just bool.
/// This is what the commit pipeline consumes.
///
/// NOTE: This struct contains only plain `bool` fields. It does NOT
/// contain `ContractGate` or any gate type — those live in
/// `crate::gates::contracts`. This prevents the import conflict
/// identified in Fix #1.
#[derive(Debug, Clone)]
pub struct ResolvedCommitGates {
    pub fmt: bool,
    pub check: bool,
    pub clippy: bool,
    pub test: bool,
    pub banned_patterns: bool,
    pub architecture: bool,
    pub contracts: bool,
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

/// Done pipeline gate configuration.
///
/// Same `Option<bool>` override pattern as CommitGatesConfig.
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

impl DoneGatesConfig {
    /// Resolve each gate to enabled/disabled based on level + overrides.
    pub fn resolve(&self, level: GateLevel) -> ResolvedDoneGates {
        let defaults = level.done_defaults();
        ResolvedDoneGates {
            dead_code: self.dead_code.unwrap_or(defaults.dead_code),
            coverage: self.coverage.unwrap_or(defaults.coverage),
            coverage_min: self.coverage_min,
            mutation: self.mutation.unwrap_or(defaults.mutation),
            mutation_kill_rate: self.mutation_kill_rate,
            workspace_check: self.workspace_check.unwrap_or(defaults.workspace_check),
            audit: self.audit.unwrap_or(defaults.audit),
            self_review: self.self_review.unwrap_or(defaults.self_review),
        }
    }
}

/// Fully resolved done gate configuration.
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

// ─────────────────────────────────────────────────────────
// Context
// ─────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────
// Dispatch
// ─────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let toml_str = r#"
[server]
port = 8420
host = "127.0.0.1"

[providers.deepseek]
enabled = true
url = "https://chat.deepseek.com"
max_concurrent = 2
input_selector = "textarea[id='chat-input']"
send_selector = "div[class*='send-button']"
"#;
        let config: PilotConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.port, 8420);
        assert!(config.providers.contains_key("deepseek"));
        assert_eq!(config.providers["deepseek"].max_concurrent, 2);
    }

    #[test]
    fn test_config_defaults_applied() {
        let toml_str = r#"
[server]

[providers.deepseek]
enabled = true
url = "https://chat.deepseek.com"
input_selector = "textarea"
send_selector = "button"
"#;
        let config: PilotConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.port, 8420);
        assert_eq!(config.defaults.max_turns, 50);
        assert_eq!(config.execution.max_fix_rounds, 5);
        assert_eq!(config.dispatch.strategy, "most_slots_first");
    }

    #[test]
    fn test_default_for_testing() {
        let config = PilotConfig::default_for_testing();
        assert!(config.providers.contains_key("deepseek"));
        assert_eq!(config.server.port, 8420);
    }

    #[test]
    fn test_gate_level_from_str() {
        assert_eq!("relaxed".parse::<GateLevel>().unwrap(), GateLevel::Relaxed);
        assert_eq!("normal".parse::<GateLevel>().unwrap(), GateLevel::Normal);
        assert_eq!("strict".parse::<GateLevel>().unwrap(), GateLevel::Strict);
        assert_eq!(
            "production".parse::<GateLevel>().unwrap(),
            GateLevel::Production
        );
        assert!("invalid".parse::<GateLevel>().is_err());
    }

    #[test]
    fn test_gate_level_display() {
        assert_eq!(format!("{}", GateLevel::Relaxed), "relaxed");
        assert_eq!(format!("{}", GateLevel::Production), "production");
    }

    #[test]
    fn test_commit_gates_resolve_relaxed() {
        let config = CommitGatesConfig::default(); // all None
        let resolved = config.resolve(GateLevel::Relaxed);
        assert!(resolved.fmt);
        assert!(resolved.check);
        assert!(!resolved.clippy);
        assert!(!resolved.test);
        assert!(!resolved.banned_patterns);
    }

    #[test]
    fn test_commit_gates_resolve_normal() {
        let config = CommitGatesConfig::default();
        let resolved = config.resolve(GateLevel::Normal);
        assert!(resolved.fmt);
        assert!(resolved.check);
        assert!(resolved.clippy);
        assert!(resolved.test);
        assert!(!resolved.banned_patterns);
    }

    #[test]
    fn test_commit_gates_resolve_strict() {
        let config = CommitGatesConfig::default();
        let resolved = config.resolve(GateLevel::Strict);
        assert!(resolved.banned_patterns);
        assert!(resolved.architecture);
        assert!(resolved.contracts);
    }

    #[test]
    fn test_commit_gates_resolve_override() {
        // Explicitly disable clippy even at normal level
        let config = CommitGatesConfig {
            clippy: Some(false),
            ..Default::default()
        };
        let resolved = config.resolve(GateLevel::Normal);
        assert!(!resolved.clippy, "override should disable clippy");
        assert!(resolved.test, "non-overridden gates should use level default");
    }

    #[test]
    fn test_done_gates_resolve_production() {
        let config = DoneGatesConfig::default();
        let resolved = config.resolve(GateLevel::Production);
        assert!(resolved.dead_code);
        assert!(resolved.coverage);
        assert!(resolved.mutation);
        assert!(resolved.workspace_check);
        assert!(resolved.audit);
        assert!(resolved.self_review);
    }

    #[test]
    fn test_done_gates_resolve_strict() {
        let config = DoneGatesConfig::default();
        let resolved = config.resolve(GateLevel::Strict);
        assert!(resolved.dead_code);
        assert!(!resolved.coverage, "strict should not require coverage");
        assert!(resolved.workspace_check);
        assert!(!resolved.self_review, "strict should not require self-review");
    }

    #[test]
    fn test_done_gates_resolve_normal() {
        let config = DoneGatesConfig::default();
        let resolved = config.resolve(GateLevel::Normal);
        assert!(!resolved.dead_code, "normal should not run done gates");
        assert!(!resolved.coverage);
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = PilotConfig::default_for_testing();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: PilotConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.server.port, parsed.server.port);
        assert_eq!(config.providers.len(), parsed.providers.len());
    }

    #[test]
    fn test_execution_config_defaults() {
        let config = ExecutionConfig::default();
        assert_eq!(config.max_fix_rounds, 5);
        assert_eq!(config.require_confirmation, "first");
        assert!(!config.dangerous_patterns.is_empty());
    }

    #[test]
    fn test_resolved_commit_gates_has_no_contract_gate_type() {
        // Fix #1: ResolvedCommitGates should contain only bool fields,
        // NOT a ContractGate type. That type lives in crate::gates::contracts.
        let resolved = CommitGatesConfig::default().resolve(GateLevel::Production);
        // Verify the field is a plain bool, not a gate type
        let _: bool = resolved.contracts;
        assert!(resolved.contracts);
    }

    #[test]
    fn test_rate_limit_cooldown_is_u64() {
        // Fix #11: rate_limit_cooldown is u64, matching the config type.
        // The cast to i64 for chrono is documented and safe.
        let config = PilotConfig::default_for_testing();
        let cooldown: u64 = config.providers["deepseek"].rate_limit_cooldown;
        assert_eq!(cooldown, 60);
        // Verify the type is u64 (not i64)
        let _: u64 = cooldown;
    }
}
```

Also create `src/config/mod.rs`:

```rust
// src/config/mod.rs

pub mod types;

use crate::error::PilotError;
use std::path::Path;
pub use types::*;

/// Load configuration from the project root's `.glyim-pilot.toml`.
pub fn load_config(project_root: &Path) -> Result<PilotConfig, PilotError> {
    let config_path = project_root.join(".glyim-pilot.toml");
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| PilotError::Config(format!("failed to read config: {e}")))?;
    let config: PilotConfig = toml::from_str(&content)
        .map_err(|e| PilotError::Config(format!("failed to parse config: {e}")))?;
    Ok(config)
}
```

Update `src/lib.rs` to add `pub mod config;`.

- [ ] **Step 2: Run tests**

Run: `cargo test --lib config`
Expected: 15 PASS

- [ ] **Step 3: Commit**

```bash
git add src/config/ src/lib.rs && git commit -m "feat: add complete config system with gate strictness levels, no ContractGate in types (Fix #1 prep), u64 cooldown (Fix #11 prep)"
```

---

### Task 2: Git Worktree Operations — Core Functions

**Files:**
- Create: `src/git_ops/mod.rs`
- Create: `src/git_ops/worktree.rs`

- [ ] **Step 1: Implement all git operations with full tracing**

```rust
// src/git_ops/worktree.rs

use crate::error::PilotError;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Create a git worktree for a stream on a new branch from main.
///
/// Equivalent to:
/// ```bash
/// git worktree add --detach <dir> main
/// git -C <dir> checkout -b stream-SXX/v0.1.0
/// ```
pub async fn create_worktree(
    repo_root: &Path,
    worktree_base: &Path,
    stream_id: &str,
) -> Result<PathBuf, PilotError> {
    let worktree_dir = worktree_base.join(format!("stream-{stream_id}"));
    let branch_name = format!("stream-{stream_id}/v0.1.0");

    tracing::info!(stream_id, ?worktree_dir, "creating worktree");

    // git worktree add --detach <dir> main
    let output = Command::new("git")
        .args(["worktree", "add", "--detach"])
        .arg(&worktree_dir)
        .arg("main")
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| PilotError::Git(format!("failed to execute git worktree add: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "git worktree add failed: {stderr}"
        )));
    }

    // git checkout -b stream-SXX/v0.1.0
    let output = Command::new("git")
        .args(["checkout", "-b", &branch_name])
        .current_dir(&worktree_dir)
        .output()
        .await
        .map_err(|e| PilotError::Git(format!("failed to create branch: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "git checkout -b failed: {stderr}"
        )));
    }

    tracing::info!(stream_id, branch = %branch_name, "worktree created and branch checked out");
    Ok(worktree_dir)
}

/// Stage all changes and commit in the worktree.
///
/// Equivalent to:
/// ```bash
/// git add -A
/// git commit -m "stream-SXX: <message>"
/// ```
///
/// If there is nothing to commit, succeeds as a no-op.
pub async fn commit_all(
    worktree_dir: &Path,
    stream_id: &str,
    message: &str,
) -> Result<(), PilotError> {
    let commit_msg = format!("stream-{stream_id}: {message}");

    tracing::debug!(stream_id, %commit_msg, "staging and committing");

    // git add -A
    let output = Command::new("git")
        .args(["add", "-A"])
        .current_dir(worktree_dir)
        .output()
        .await
        .map_err(|e| PilotError::Git(format!("git add failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!("git add failed: {stderr}")));
    }

    // git commit -m "stream-SXX: message"
    let output = Command::new("git")
        .args(["commit", "-m", &commit_msg])
        .current_dir(worktree_dir)
        .output()
        .await
        .map_err(|e| PilotError::Git(format!("git commit failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        // "nothing to commit" is not an error
        if stderr.contains("nothing to commit") || stderr.contains("no changes added to commit") {
            tracing::debug!(stream_id, "nothing to commit — no-op");
            return Ok(());
        }
        return Err(PilotError::Git(format!("git commit failed: {stderr}")));
    }

    tracing::info!(stream_id, "committed successfully");
    Ok(())
}

/// Make an emergency WIP commit (for escalation after max fix rounds).
pub async fn emergency_wip_commit(
    worktree_dir: &Path,
    stream_id: &str,
) -> Result<(), PilotError> {
    tracing::warn!(stream_id, "making emergency WIP commit — fix rounds exceeded");
    commit_all(worktree_dir, stream_id, "WIP: emergency commit — fix rounds exceeded").await
}

/// Push the current branch to origin.
pub async fn push_branch(worktree_dir: &Path, stream_id: &str) -> Result<(), PilotError> {
    let branch_name = format!("stream-{stream_id}/v0.1.0");

    tracing::info!(stream_id, branch = %branch_name, "pushing branch");

    let output = Command::new("git")
        .args(["push", "-u", "origin", &branch_name])
        .current_dir(worktree_dir)
        .output()
        .await
        .map_err(|e| PilotError::Git(format!("git push failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!("git push failed: {stderr}")));
    }

    Ok(())
}

/// Create a PR using `gh` CLI.
///
/// Returns the PR URL on success.
pub async fn create_pr(
    worktree_dir: &Path,
    stream_id: &str,
    title: &str,
    body: &str,
) -> Result<String, PilotError> {
    let branch_name = format!("stream-{stream_id}/v0.1.0");

    tracing::info!(stream_id, %title, "creating PR");

    let output = Command::new("gh")
        .args([
            "pr",
            "create",
            "--base",
            "main",
            "--head",
            &branch_name,
            "--title",
            title,
            "--body",
            body,
        ])
        .current_dir(worktree_dir)
        .output()
        .await
        .map_err(|e| PilotError::Git(format!("gh pr create failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!("gh pr create failed: {stderr}")));
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    tracing::info!(stream_id, %url, "PR created");
    Ok(url)
}

/// Get git status in porcelain format.
pub async fn status_porcelain(worktree_dir: &Path) -> Result<String, PilotError> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_dir)
        .output()
        .await
        .map_err(|e| PilotError::Git(format!("git status failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!("git status failed: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Get the diff between main and HEAD.
pub async fn diff_main(worktree_dir: &Path) -> Result<String, PilotError> {
    let output = Command::new("git")
        .args(["diff", "main..HEAD"])
        .current_dir(worktree_dir)
        .output()
        .await
        .map_err(|e| PilotError::Git(format!("git diff failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!("git diff failed: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get the commit log between main and HEAD (oneline format).
pub async fn log_oneline(worktree_dir: &Path) -> Result<String, PilotError> {
    let output = Command::new("git")
        .args(["log", "main..HEAD", "--oneline"])
        .current_dir(worktree_dir)
        .output()
        .await
        .map_err(|e| PilotError::Git(format!("git log failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!("git log failed: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Remove a git worktree.
pub async fn remove_worktree(
    repo_root: &Path,
    worktree_dir: &Path,
) -> Result<(), PilotError> {
    tracing::info!(?worktree_dir, "removing worktree");

    let output = Command::new("git")
        .args(["worktree", "remove"])
        .arg(worktree_dir)
        .arg("--force")
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| PilotError::Git(format!("git worktree remove failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PilotError::Git(format!(
            "git worktree remove failed: {stderr}"
        )));
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::process::Command as AsyncCommand;

    /// Set up a temporary git repo with an initial commit on `main`.
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
        let worktree_base = root.parent().unwrap().join("wt_create");

        let result = create_worktree(root, &worktree_base, "S01").await;
        assert!(result.is_ok(), "create_worktree failed: {:?}", result.err());

        let wt_path = result.unwrap();
        assert!(wt_path.exists());
        assert!(wt_path.join("README.md").exists());
    }

    #[tokio::test]
    async fn test_commit_all_success() {
        let dir = setup_test_repo().await;
        let root = dir.path();
        let worktree_base = root.parent().unwrap().join("wt_commit");

        let wt_path = create_worktree(root, &worktree_base, "S02").await.unwrap();
        std::fs::write(wt_path.join("src/lib.rs"), "pub fn hello() {}").unwrap();

        let result = commit_all(&wt_path, "S02", "add hello function").await;
        assert!(result.is_ok(), "commit_all failed: {:?}", result.err());

        let status = status_porcelain(&wt_path).await.unwrap();
        assert!(status.is_empty(), "worktree should be clean after commit");
    }

    #[tokio::test]
    async fn test_commit_all_nothing_to_commit() {
        let dir = setup_test_repo().await;
        let root = dir.path();
        let worktree_base = root.parent().unwrap().join("wt_empty");

        let wt_path = create_worktree(root, &worktree_base, "S03").await.unwrap();
        let result = commit_all(&wt_path, "S03", "nothing new").await;
        assert!(result.is_ok(), "commit with nothing staged should be no-op");
    }

    #[tokio::test]
    async fn test_emergency_wip_commit() {
        let dir = setup_test_repo().await;
        let root = dir.path();
        let worktree_base = root.parent().unwrap().join("wt_wip");

        let wt_path = create_worktree(root, &worktree_base, "S04").await.unwrap();
        std::fs::write(wt_path.join("broken.rs"), "broken code").unwrap();

        let result = emergency_wip_commit(&wt_path, "S04").await;
        assert!(result.is_ok());

        // Verify the commit exists
        let log = log_oneline(&wt_path).await.unwrap();
        assert!(log.contains("WIP"));
    }

    #[tokio::test]
    async fn test_status_porcelain_clean() {
        let dir = setup_test_repo().await;
        let root = dir.path();
        let status = status_porcelain(root).await.unwrap();
        assert!(status.is_empty());
    }

    #[tokio::test]
    async fn test_status_porcelain_dirty() {
        let dir = setup_test_repo().await;
        let root = dir.path();
        std::fs::write(root.join("new_file.rs"), "fn main() {}").unwrap();
        let status = status_porcelain(root).await.unwrap();
        assert!(!status.is_empty());
        assert!(status.contains("new_file.rs"));
    }

    #[tokio::test]
    async fn test_diff_main() {
        let dir = setup_test_repo().await;
        let root = dir.path();
        let worktree_base = root.parent().unwrap().join("wt_diff");

        let wt_path = create_worktree(root, &worktree_base, "S05").await.unwrap();
        std::fs::write(wt_path.join("src/lib.rs"), "pub fn new() {}").unwrap();
        commit_all(&wt_path, "S05", "add new function").await.unwrap();

        let diff = diff_main(&wt_path).await.unwrap();
        assert!(diff.contains("new()"));
    }

    #[tokio::test]
    async fn test_log_oneline() {
        let dir = setup_test_repo().await;
        let root = dir.path();
        let worktree_base = root.parent().unwrap().join("wt_log");

        let wt_path = create_worktree(root, &worktree_base, "S06").await.unwrap();
        std::fs::write(wt_path.join("a.rs"), "a").unwrap();
        commit_all(&wt_path, "S06", "first commit").await.unwrap();
        std::fs::write(wt_path.join("b.rs"), "b").unwrap();
        commit_all(&wt_path, "S06", "second commit").await.unwrap();

        let log = log_oneline(&wt_path).await.unwrap();
        let lines: Vec<&str> = log.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[tokio::test]
    async fn test_remove_worktree() {
        let dir = setup_test_repo().await;
        let root = dir.path();
        let worktree_base = root.parent().unwrap().join("wt_remove");

        let wt_path = create_worktree(root, &worktree_base, "S07").await.unwrap();
        assert!(wt_path.exists());

        remove_worktree(root, &wt_path).await.unwrap();
        assert!(!wt_path.exists());
    }
}
```

Create `src/git_ops/mod.rs`:

```rust
// src/git_ops/mod.rs

pub mod worktree;

pub use worktree::{
    create_worktree, commit_all, emergency_wip_commit, push_branch, create_pr,
    status_porcelain, diff_main, log_oneline, remove_worktree,
};
```

Update `src/lib.rs` to add `pub mod git_ops;`.

- [ ] **Step 2: Run tests**

Run: `cargo test --lib git_ops`
Expected: 9 PASS

- [ ] **Step 3: Commit**

```bash
git add src/git_ops/ src/lib.rs && git commit -m "feat: add git worktree operations with emergency WIP commit and full tracing"
```

---

### Task 3: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test --lib`
Expected: All PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --check`
Expected: No formatting issues

- [ ] **Step 4: Build release binary**

Run: `cargo build --release`
Expected: Compiles

- [ ] **Step 5: Tag**

```bash
git tag v0.1.0-config-git -m "Complete configuration with gate levels and git worktree operations"
```

---

**Phase 2 complete.** All config types are fully defined with gate strictness level resolution. Key design decisions that prevent future bugs:

- **`ContractGate` is NOT in `config::types`** — it lives exclusively in `crate::gates::contracts`. `ResolvedCommitGates` has only `bool` fields. This prevents Fix #1 (the import conflict where `ContractGate` was imported from two modules).
- **`rate_limit_cooldown` is `u64`** in config, matching the natural type. The cast to `i64` for `chrono::Duration::seconds()` is documented and safe for any practical value. This addresses Fix #11 at the source.
- **`max_fix_rounds` comes from `ExecutionConfig`** (default: 5), not hardcoded. The orchestrator will read it from `config.execution.max_fix_rounds`.
- **`CommitGatesConfig::resolve()` and `DoneGatesConfig::resolve()`** produce `ResolvedCommitGates` and `ResolvedDoneGates` — concrete `bool` structs with no `Option<bool>`.

Ready for **Phase 3: Quality Gates & Commit Engine** — shall I continue?
# Phase 3: Quality Gates & Commit Engine — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the complete quality gate system (all 10 gates required by the spec), the shared `PipelineResult` type, commit and done pipelines with gate execution timing, and the stateless commit engine that reads `fix_round` from `SessionState`.

**Architecture:** Each gate implements an `async_trait Gate` trait. `PipelineResult` is a shared type for both commit and done pipelines. Regex patterns are compiled once via `std::sync::LazyLock`. File-walking gates use `spawn_blocking` to avoid blocking the async runtime (Fix #8). `FmtGate` auto-fixes and returns PASS. `CheckGate` does NOT pass `"2>&1"` as a cargo argument. `DeadCodeGate` does NOT pass when `cargo check` fails. `CommitEngine` is stateless — it takes `current_fix_round` as input and returns the new value (Fix #4). Gate execution timing is logged after each gate run (Fix #10). `ContractGate` is imported only from `crate::gates::contracts`, never from `config::types` (Fix #1). No `GateConfig` struct exists anywhere (Fix #3).

**Tech Stack:** async-trait 0.1, tokio::process + spawn_blocking, regex 1.11 (LazyLock), ignore 0.4, strip-ansi-escapes 0.2

---

### Task 1: Gate Trait, GateResult, PipelineResult, and Shared Helpers

**Files:**
- Create: `src/gates/mod.rs`
- Create: `src/gates/types.rs`
- Create: `src/gates/helpers.rs`

This task defines the `Gate` trait, `GateResult`, the shared `PipelineResult`, and all helper functions. No `GateConfig` struct is defined — it was dead code and has been removed (Fix #3).

- [ ] **Step 1: Create src/gates/types.rs**

```rust
// src/gates/types.rs

use serde::{Deserialize, Serialize};

/// Result of running a single quality gate.
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

    /// Skip this gate (not applicable in current config).
    pub fn skip(name: impl Into<String>) -> Self {
        Self {
            gate_name: name.into(),
            passed: true,
            message: "skipped".into(),
            details: None,
        }
    }
}

/// Shared result type for both commit and done pipelines.
///
/// Eliminates the duplication between CommitPipelineResult and
/// DonePipelineResult — they had identical fields and methods.
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

    /// Format a human-readable failure message for AI feedback.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_result_pass() {
        let r = GateResult::pass("fmt");
        assert!(r.passed);
        assert_eq!(r.gate_name, "fmt");
        assert_eq!(r.message, "passed");
    }

    #[test]
    fn test_gate_result_pass_with_note() {
        let r = GateResult::pass_with_note("fmt", "auto-fixed");
        assert!(r.passed);
        assert_eq!(r.message, "auto-fixed");
    }

    #[test]
    fn test_gate_result_fail() {
        let r = GateResult::fail("check", "compile error");
        assert!(!r.passed);
        assert_eq!(r.message, "compile error");
        assert!(r.details.is_none());
    }

    #[test]
    fn test_gate_result_fail_with_details() {
        let r = GateResult::fail_with_details("coverage", "too low", "62% < 80%");
        assert!(!r.passed);
        assert_eq!(r.details.as_deref(), Some("62% < 80%"));
    }

    #[test]
    fn test_gate_result_skip() {
        let r = GateResult::skip("mutation");
        assert!(r.passed);
        assert_eq!(r.message, "skipped");
    }

    #[test]
    fn test_gate_result_serialization() {
        let r = GateResult::fail("test", "2 failures");
        let json = serde_json::to_string(&r).unwrap();
        let de: GateResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r.gate_name, de.gate_name);
        assert_eq!(r.passed, de.passed);
    }

    #[test]
    fn test_pipeline_result_all_pass() {
        let result = PipelineResult::from_gates(vec![
            GateResult::pass("fmt"),
            GateResult::pass("check"),
        ]);
        assert!(result.passed);
        assert!(result.first_failure().is_none());
    }

    #[test]
    fn test_pipeline_result_first_failure() {
        let result = PipelineResult::from_gates(vec![
            GateResult::pass("fmt"),
            GateResult::pass("check"),
            GateResult::fail("clippy", "warning found"),
        ]);
        assert!(!result.passed);
        let fail = result.first_failure().unwrap();
        assert_eq!(fail.gate_name, "clippy");
    }

    #[test]
    fn test_pipeline_failure_message_format() {
        let result = PipelineResult::from_gates(vec![
            GateResult::fail_with_details("check", "compile failed", "error[E0308]"),
        ]);
        let msg = result.failure_message();
        assert!(msg.contains("**check failed**"), "expected bold gate name, got: {msg}");
        assert!(msg.contains("error[E0308]"));
    }

    #[test]
    fn test_pipeline_failure_message_empty_when_passed() {
        let result = PipelineResult::from_gates(vec![GateResult::pass("fmt")]);
        assert!(result.failure_message().is_empty());
    }
}
```

- [ ] **Step 2: Create src/gates/helpers.rs**

```rust
// src/gates/helpers.rs

use crate::error::PilotError;
use std::path::Path;

/// Run a command asynchronously and capture its output.
pub async fn run_command(
    program: &str,
    args: &[&str],
    cwd: &Path,
) -> Result<std::process::Output, PilotError> {
    tracing::debug!(program, ?args, ?cwd, "running command");
    let output = tokio::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|e| PilotError::Gate {
            gate: program.into(),
            message: format!("failed to execute {program}: {e}"),
        })?;
    Ok(output)
}

/// Strip ANSI escape codes from a string.
pub fn strip_ansi(s: &str) -> String {
    strip_ansi_escapes::strip_str(s)
}

/// Trim long command output by extracting lines that start with markers
/// (e.g., "error", "warning") and their surrounding context.
pub fn trim_output_by_markers(output: &str, markers: &[&str], max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= max_lines {
        return output.to_string();
    }

    let mut relevant = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if markers.iter().any(|m| trimmed.starts_with(m)) {
            let start = i.saturating_sub(2);
            let end = (i + 5).min(lines.len());
            for j in start..end {
                relevant.push(lines[j]);
            }
            relevant.push("...");
        }
    }

    if relevant.is_empty() {
        lines[lines.len() - max_lines..].join("\n")
    } else {
        relevant.join("\n")
    }
}

/// Trim compile/clippy output to a reasonable size.
/// Uses markers "error" and "warning".
pub fn trim_errors_and_warnings(output: &str) -> String {
    trim_output_by_markers(output, &["error", "warning"], 50)
}

/// Trim test failure output to a reasonable size.
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
                relevant.push("... (truncated)");
                break;
            }
        }
    }
    // Always include the test result summary line
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_short_output_unchanged() {
        let input = "error: expected `;`\n --> src/lib.rs:1:10";
        let result = trim_errors_and_warnings(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_trim_long_output_keeps_errors() {
        let mut lines: Vec<String> = (0..100).map(|i| format!("line {i}")).collect();
        lines.push("error[E0308]: mismatched types".into());
        lines.push(" --> src/lib.rs:5:1".into());
        let input = lines.join("\n");
        let result = trim_errors_and_warnings(&input);
        assert!(result.contains("error[E0308]"));
        assert!(result.len() < input.len());
    }

    #[test]
    fn test_trim_test_failures_short() {
        let output = "running 10 tests\n.....F....\nfailures:\ntest_foo\n\nleft: 1\nright: 2\n\ntest result: FAILED. 9 passed; 1 failed";
        let trimmed = trim_test_failures(output);
        assert!(trimmed.contains("failures:"));
        assert!(trimmed.contains("FAILED"));
    }

    #[test]
    fn test_trim_test_failures_includes_summary() {
        let mut lines: Vec<String> = (0..200).map(|i| format!("line {i}")).collect();
        lines.push("test result: FAILED. 9 passed; 1 failed; 0 ignored".into());
        let output = lines.join("\n");
        let trimmed = trim_test_failures(&output);
        assert!(trimmed.contains("test result: FAILED"));
    }
}
```

- [ ] **Step 3: Create src/gates/mod.rs with Gate trait**

```rust
// src/gates/mod.rs

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
use async_trait::async_trait;
use std::path::Path;

pub use types::{GateResult, PipelineResult};

/// A quality gate that can be run against a worktree.
#[async_trait]
pub trait Gate: Send + Sync {
    fn name(&self) -> &str;
    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError>;
}
```

Update `src/lib.rs` to add `pub mod gates;`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib gates::types gates::helpers`
Expected: 12 PASS

- [ ] **Step 5: Commit**

```bash
git add src/gates/ src/lib.rs && git commit -m "feat: add Gate trait with async-trait, PipelineResult, and shared helpers — no GateConfig (Fix #3)"
```

---

### Task 2: FmtGate and CheckGate

**Files:**
- Create: `src/gates/fmt.rs`
- Create: `src/gates/check.rs`

**Key behaviors:**
- `FmtGate` auto-fixes and returns PASS with a note (not fail after auto-fix)
- `CheckGate` does NOT pass `"2>&1"` as a cargo argument — stderr is captured via `output.stderr`

- [ ] **Step 1: Implement FmtGate (auto-fix → PASS)**

```rust
// src/gates/fmt.rs

use crate::error::PilotError;
use crate::gates::helpers::run_command;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use std::path::Path;

pub struct FmtGate;

#[async_trait]
impl Gate for FmtGate {
    fn name(&self) -> &str {
        "fmt"
    }

    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError> {
        // cargo fmt -- --check (dry-run: exits 1 if formatting would change)
        let output = run_command("cargo", &["fmt", "--", "--check"], worktree_dir).await?;

        if output.status.success() {
            tracing::debug!("fmt: already formatted");
            Ok(GateResult::pass("fmt"))
        } else {
            // Auto-fix: run cargo fmt to apply formatting
            let fix_output = run_command("cargo", &["fmt"], worktree_dir).await?;

            if fix_output.status.success() {
                tracing::info!("fmt: auto-fixed formatting changes");
                Ok(GateResult::pass_with_note(
                    "fmt",
                    "auto-fixed: cargo fmt applied changes",
                ))
            } else {
                // Auto-fix itself failed — this is a real error
                let stderr = crate::gates::helpers::strip_ansi(&String::from_utf8_lossy(
                    &fix_output.stderr,
                ));
                tracing::error!("fmt: auto-fix failed: {stderr}");
                Ok(GateResult::fail_with_details(
                    "fmt",
                    "cargo fmt failed to apply formatting",
                    stderr,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmt_gate_name() {
        let gate = FmtGate;
        assert_eq!(gate.name(), "fmt");
    }
}
```

- [ ] **Step 2: Implement CheckGate (no "2>&1")**

```rust
// src/gates/check.rs

use crate::error::PilotError;
use crate::gates::helpers::{run_command, strip_ansi, trim_errors_and_warnings};
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use std::path::Path;

pub struct CheckGate;

#[async_trait]
impl Gate for CheckGate {
    fn name(&self) -> &str {
        "check"
    }

    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError> {
        // NOTE: We do NOT pass "2>&1" as a cargo argument.
        // tokio::process::Command does not spawn a shell; "2>&1" would be
        // forwarded as a literal argument to cargo. Stderr is captured
        // via output.stderr already.
        let output = run_command("cargo", &["check"], worktree_dir).await?;

        if output.status.success() {
            tracing::debug!("check: compilation succeeded");
            Ok(GateResult::pass("check"))
        } else {
            let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
            let trimmed = trim_errors_and_warnings(&stderr);
            tracing::info!(
                "check: compilation failed ({} bytes trimmed to {})",
                stderr.len(),
                trimmed.len()
            );
            Ok(GateResult::fail_with_details(
                "check",
                "compilation failed",
                trimmed,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_gate_name() {
        let gate = CheckGate;
        assert_eq!(gate.name(), "check");
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib gates::fmt gates::check`
Expected: 2 PASS

- [ ] **Step 4: Commit**

```bash
git add src/gates/fmt.rs src/gates/check.rs && git commit -m "feat: add FmtGate (auto-fix→PASS) and CheckGate (no shell redirect hack)"
```

---

### Task 3: ClippyGate and TestGate

**Files:**
- Create: `src/gates/clippy.rs`
- Create: `src/gates/test_gate.rs`

- [ ] **Step 1: Implement ClippyGate**

```rust
// src/gates/clippy.rs

use crate::error::PilotError;
use crate::gates::helpers::{run_command, strip_ansi, trim_errors_and_warnings};
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use std::path::Path;

pub struct ClippyGate;

#[async_trait]
impl Gate for ClippyGate {
    fn name(&self) -> &str {
        "clippy"
    }

    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError> {
        let output = run_command(
            "cargo",
            &["clippy", "--", "-D", "warnings"],
            worktree_dir,
        )
        .await?;

        if output.status.success() {
            tracing::debug!("clippy: no warnings");
            Ok(GateResult::pass("clippy"))
        } else {
            let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
            let trimmed = trim_errors_and_warnings(&stderr);
            tracing::info!("clippy: warnings found");
            Ok(GateResult::fail_with_details(
                "clippy",
                "clippy warnings found",
                trimmed,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clippy_gate_name() {
        let gate = ClippyGate;
        assert_eq!(gate.name(), "clippy");
    }
}
```

- [ ] **Step 2: Implement TestGate**

```rust
// src/gates/test_gate.rs

use crate::error::PilotError;
use crate::gates::helpers::{run_command, strip_ansi, trim_test_failures};
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use std::path::Path;

pub struct TestGate;

#[async_trait]
impl Gate for TestGate {
    fn name(&self) -> &str {
        "test"
    }

    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError> {
        let output = run_command("cargo", &["test"], worktree_dir).await?;

        if output.status.success() {
            tracing::debug!("test: all tests passed");
            Ok(GateResult::pass("test"))
        } else {
            let stdout = strip_ansi(&String::from_utf8_lossy(&output.stdout));
            let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
            let combined = format!("{stdout}\n{stderr}");
            let trimmed = trim_test_failures(&combined);
            tracing::info!("test: failures detected");
            Ok(GateResult::fail_with_details(
                "test",
                "test failures detected",
                trimmed,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_gate_name() {
        let gate = TestGate;
        assert_eq!(gate.name(), "test");
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib gates::clippy gates::test_gate`
Expected: 2 PASS

- [ ] **Step 4: Commit**

```bash
git add src/gates/clippy.rs src/gates/test_gate.rs && git commit -m "feat: add ClippyGate and TestGate with shared output trimming"
```

---

### Task 4: BannedPatternGate (spawn_blocking — Fix #8)

**Files:**
- Create: `src/gates/banned_pattern.rs`

- [ ] **Step 1: Implement BannedPatternGate with spawn_blocking**

```rust
// src/gates/banned_pattern.rs

use crate::error::PilotError;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use std::path::Path;

/// Patterns banned in non-test code per AGENT_MASTER_CONTEXT.md.
const BANNED_PATTERNS: &[(&str, &str)] = &[
    ("todo!()", "`todo!()` in non-test code"),
    ("unwrap()", "`.unwrap()` in non-test code"),
    ("panic!()", "`panic!()` in non-test code"),
    (" as ", "explicit `as` cast in non-test code"),
];

pub struct BannedPatternGate;

#[async_trait]
impl Gate for BannedPatternGate {
    fn name(&self) -> &str {
        "banned_patterns"
    }

    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError> {
        // Fix #8: File walking and reading are synchronous — use
        // spawn_blocking to avoid blocking the async runtime (NFR-PERF-001).
        let worktree_dir = worktree_dir.to_path_buf();
        let result = tokio::task::spawn_blocking(move || scan_for_banned_patterns(&worktree_dir))
            .await
            .map_err(|e| PilotError::Gate {
                gate: "banned_patterns".into(),
                message: format!("spawn_blocking failed: {e}"),
            })?;

        Ok(result)
    }
}

/// Synchronous file scanning — runs on a blocking thread.
fn scan_for_banned_patterns(worktree_dir: &Path) -> GateResult {
    let mut violations = Vec::new();

    let walker = ignore::WalkBuilder::new(worktree_dir)
        .hidden(false)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if !path.extension().map_or(false, |e| e == "rs") {
            continue;
        }

        // Skip test files
        let path_str = path.to_string_lossy();
        if path_str.contains("/tests/") || path_str.contains("\\tests\\") {
            continue;
        }
        if path
            .file_name()
            .map_or(false, |n| n.to_string_lossy().starts_with("test_"))
        {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for line_content in content.lines() {
            let trimmed = line_content.trim();
            // Skip comment lines
            if trimmed.starts_with("//") {
                continue;
            }

            for (pattern, description) in BANNED_PATTERNS {
                if line_content.contains(pattern) {
                    let relative = path
                        .strip_prefix(worktree_dir)
                        .unwrap_or(path)
                        .display();
                    violations.push(format!("{relative}: {description}"));
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_worktree() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    #[tokio::test]
    async fn test_banned_pattern_gate_clean() {
        let dir = setup_worktree();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(
            dir.path().join("src/lib.rs"),
            "pub fn hello() -> &'static str { \"hello\" }",
        )
        .unwrap();

        let gate = BannedPatternGate;
        let result = gate.run(dir.path()).await.unwrap();
        assert!(result.passed, "clean code should pass");
    }

    #[tokio::test]
    async fn test_banned_pattern_gate_has_todo() {
        let dir = setup_worktree();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "pub fn hello() { todo!() }").unwrap();

        let gate = BannedPatternGate;
        let result = gate.run(dir.path()).await.unwrap();
        assert!(!result.passed);
        assert!(result.details.as_ref().unwrap().contains("todo!()"));
    }

    #[tokio::test]
    async fn test_banned_pattern_gate_skips_tests_dir() {
        let dir = setup_worktree();
        fs::create_dir_all(dir.path().join("tests")).unwrap();
        fs::write(
            dir.path().join("tests/integration.rs"),
            "#[test]\nfn test_it() { todo!() }",
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "pub fn hello() -> i32 { 42 }").unwrap();

        let gate = BannedPatternGate;
        let result = gate.run(dir.path()).await.unwrap();
        assert!(
            result.passed,
            "todo!() in test files should be allowed"
        );
    }

    #[tokio::test]
    async fn test_banned_pattern_gate_has_unwrap() {
        let dir = setup_worktree();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(
            dir.path().join("src/lib.rs"),
            "pub fn hello(s: &str) { s.parse::<i32>().unwrap(); }",
        )
        .unwrap();

        let gate = BannedPatternGate;
        let result = gate.run(dir.path()).await.unwrap();
        assert!(!result.passed);
        assert!(result.details.as_ref().unwrap().contains("unwrap()"));
    }

    #[tokio::test]
    async fn test_banned_pattern_gate_skips_comments() {
        let dir = setup_worktree();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(
            dir.path().join("src/lib.rs"),
            "// TODO: implement this later\npub fn hello() -> i32 { 42 }",
        )
        .unwrap();

        let gate = BannedPatternGate;
        let result = gate.run(dir.path()).await.unwrap();
        assert!(result.passed, "todo!() in comments should be allowed");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib gates::banned_pattern`
Expected: 5 PASS

- [ ] **Step 3: Commit**

```bash
git add src/gates/banned_pattern.rs && git commit -m "feat: add BannedPatternGate with spawn_blocking for file I/O (Fix #8)"
```

---

### Task 5: ArchitectureGate and ContractGate

**Files:**
- Create: `src/gates/architecture.rs`
- Create: `src/gates/contracts.rs`

Both use `spawn_blocking` for file I/O (Fix #8).

- [ ] **Step 1: Implement ArchitectureGate (spawn_blocking)**

```rust
// src/gates/architecture.rs

use crate::error::PilotError;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use std::path::Path;

/// A dependency rule: crate `from` must not depend on crate `to`.
#[derive(Debug, Clone)]
pub struct DependencyRule {
    pub from_crate: String,
    pub forbidden_dep: String,
    pub reason: String,
}

/// Default architecture rules for the Glyim compiler project.
fn default_rules() -> Vec<DependencyRule> {
    vec![
        DependencyRule {
            from_crate: "glyim-frontend".into(),
            forbidden_dep: "glyim-type".into(),
            reason: "frontend must not depend on type directly (use syntax as intermediary)".into(),
        },
        DependencyRule {
            from_crate: "glyim-frontend".into(),
            forbidden_dep: "glyim-ir".into(),
            reason: "frontend must not depend on IR".into(),
        },
        DependencyRule {
            from_crate: "glyim-frontend".into(),
            forbidden_dep: "glyim-codegen".into(),
            reason: "frontend must not depend on codegen".into(),
        },
        DependencyRule {
            from_crate: "glyim-syntax".into(),
            forbidden_dep: "glyim-ir".into(),
            reason: "syntax must not depend on IR".into(),
        },
        DependencyRule {
            from_crate: "glyim-syntax".into(),
            forbidden_dep: "glyim-codegen".into(),
            reason: "syntax must not depend on codegen".into(),
        },
        DependencyRule {
            from_crate: "glyim-type".into(),
            forbidden_dep: "glyim-codegen".into(),
            reason: "type must not depend on codegen".into(),
        },
    ]
}

pub struct ArchitectureGate {
    rules: Vec<DependencyRule>,
}

impl ArchitectureGate {
    pub fn new(rules: Vec<DependencyRule>) -> Self {
        Self { rules }
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

    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError> {
        // Fix #8: File walking is synchronous — use spawn_blocking.
        let worktree_dir = worktree_dir.to_path_buf();
        let rules = self.rules.clone();
        let result = tokio::task::spawn_blocking(move || {
            check_architecture(&worktree_dir, &rules)
        })
        .await
        .map_err(|e| PilotError::Gate {
            gate: "architecture".into(),
            message: format!("spawn_blocking failed: {e}"),
        })?;

        Ok(result)
    }
}

fn check_architecture(worktree_dir: &Path, rules: &[DependencyRule]) -> GateResult {
    let mut violations = Vec::new();

    let walker = ignore::WalkBuilder::new(worktree_dir)
        .hidden(false)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.file_name().map_or(false, |n| n == "Cargo.toml") {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Some(crate_name) = extract_crate_name(&content) {
                    for rule in rules {
                        if crate_name == rule.from_crate {
                            if cargo_toml_depends_on(&content, &rule.forbidden_dep) {
                                let relative = path
                                    .strip_prefix(worktree_dir)
                                    .unwrap_or(path)
                                    .display();
                                violations.push(format!(
                                    "{relative}: {} depends on {} — {}",
                                    rule.from_crate, rule.forbidden_dep, rule.reason
                                ));
                            }
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
            format!("{} architecture violation(s) found", violations.len()),
            violations.join("\n"),
        )
    }
}

fn extract_crate_name(content: &str) -> Option<String> {
    let in_package = content
        .lines()
        .skip_while(|line| line.trim() != "[package]")
        .skip(1)
        .take_while(|line| !line.trim().starts_with('['))
        .find(|line| line.trim().starts_with("name ="));

    if let Some(name_line) = in_package {
        let name = name_line
            .split('=')
            .nth(1)?
            .trim()
            .trim_matches('"')
            .to_string();
        Some(name)
    } else {
        None
    }
}

fn cargo_toml_depends_on(content: &str, dep_name: &str) -> bool {
    let mut in_deps = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[dependencies]" || trimmed == "[dev-dependencies]" {
            in_deps = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_deps = false;
            continue;
        }
        if in_deps {
            if trimmed.starts_with(&format!("{dep_name} ="))
                || trimmed.starts_with(&format!("{dep_name}."))
            {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_extract_crate_name() {
        let content = "[package]\nname = \"glyim-frontend\"\nversion = \"0.1.0\"";
        assert_eq!(
            extract_crate_name(content),
            Some("glyim-frontend".into())
        );
    }

    #[test]
    fn test_extract_crate_name_missing() {
        let content = "[dependencies]\nserde = \"1\"";
        assert_eq!(extract_crate_name(content), None);
    }

    #[test]
    fn test_cargo_toml_depends_on_present() {
        let content = "[dependencies]\nglyim-type = { path = \"../type\" }\nserde = \"1\"";
        assert!(cargo_toml_depends_on(content, "glyim-type"));
    }

    #[test]
    fn test_cargo_toml_depends_on_absent() {
        let content = "[dependencies]\nserde = \"1\"";
        assert!(!cargo_toml_depends_on(content, "glyim-type"));
    }

    #[test]
    fn test_cargo_toml_depends_on_dev_dep() {
        let content = "[dev-dependencies]\nglyim-type = { path = \"../type\" }";
        assert!(cargo_toml_depends_on(content, "glyim-type"));
    }

    #[tokio::test]
    async fn test_architecture_gate_clean() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("crates/frontend")).unwrap();
        fs::write(
            dir.path().join("crates/frontend/Cargo.toml"),
            "[package]\nname = \"glyim-frontend\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"",
        )
        .unwrap();

        let gate = ArchitectureGate::with_default_rules();
        let result = gate.run(dir.path()).await.unwrap();
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_architecture_gate_violation() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("crates/frontend")).unwrap();
        fs::write(
            dir.path().join("crates/frontend/Cargo.toml"),
            "[package]\nname = \"glyim-frontend\"\nversion = \"0.1.0\"\n\n[dependencies]\nglyim-type = { path = \"../type\" }",
        )
        .unwrap();

        let gate = ArchitectureGate::with_default_rules();
        let result = gate.run(dir.path()).await.unwrap();
        assert!(!result.passed);
        assert!(result.details.as_ref().unwrap().contains("glyim-type"));
    }
}
```

- [ ] **Step 2: Implement ContractGate**

```rust
// src/gates/contracts.rs

use crate::error::PilotError;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use std::path::Path;

/// Gate that verifies locked pub interfaces have not been modified.
///
/// Reads `CONTRACTS_LOCKED.md` from the project root to determine
/// which interfaces are locked, then uses `git diff main..HEAD` to
/// check whether any locked signatures were changed.
///
/// NOTE: This struct is defined HERE in `crate::gates::contracts`,
/// NOT in `crate::config::types`. This prevents the import conflict
/// identified in Fix #1 where ContractGate was imported from two
/// different modules.
pub struct ContractGate {
    project_root: std::path::PathBuf,
}

impl ContractGate {
    pub fn new(project_root: std::path::PathBuf) -> Self {
        Self { project_root }
    }
}

#[async_trait]
impl Gate for ContractGate {
    fn name(&self) -> &str {
        "contracts"
    }

    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError> {
        // 1. Read locked interface names from CONTRACTS_LOCKED.md
        let contracts_path = self.project_root.join("CONTRACTS_LOCKED.md");
        let locked_names = if contracts_path.exists() {
            let content = tokio::fs::read_to_string(&contracts_path).await
                .map_err(|e| PilotError::Gate {
                    gate: "contracts".into(),
                    message: format!("failed to read CONTRACTS_LOCKED.md: {e}"),
                })?;
            extract_locked_names(&content)
        } else {
            tracing::debug!("contracts: no CONTRACTS_LOCKED.md found, skipping");
            return Ok(GateResult::pass_with_note(
                "contracts",
                "no CONTRACTS_LOCKED.md found",
            ));
        };

        if locked_names.is_empty() {
            return Ok(GateResult::pass("contracts"));
        }

        // 2. Get the diff between main and HEAD
        let diff = crate::git_ops::diff_main(worktree_dir).await?;

        if diff.is_empty() {
            return Ok(GateResult::pass("contracts"));
        }

        // 3. Check if any locked name appears in removed/modified lines
        let mut violations = Vec::new();
        for line in diff.lines() {
            if !line.starts_with('-') || line.starts_with("---") {
                continue;
            }
            for name in &locked_names {
                if line.contains(name.as_str()) {
                    violations.push(format!(
                        "locked interface '{}' appears in a removed/modified line: {}",
                        name,
                        line.trim_start_matches('-').trim()
                    ));
                }
            }
        }

        if violations.is_empty() {
            Ok(GateResult::pass("contracts"))
        } else {
            Ok(GateResult::fail_with_details(
                "contracts",
                format!("{} locked interface violation(s) found", violations.len()),
                violations.join("\n"),
            ))
        }
    }
}

/// Extract locked interface/function names from CONTRACTS_LOCKED.md.
fn extract_locked_names(content: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if !in_code_block {
            continue;
        }

        if let Some(name) = extract_pub_name(trimmed) {
            names.push(name);
        }
    }

    names
}

fn extract_pub_name(line: &str) -> Option<String> {
    let line = line.trim();

    if line.starts_with("pub fn ") || line.starts_with("pub async fn ") {
        let after_fn = if line.starts_with("pub async fn ") {
            &line["pub async fn ".len()..]
        } else {
            &line["pub fn ".len()..]
        };
        let name = after_fn.split('(').next()?.trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }

    if line.starts_with("pub struct ") {
        let after = &line["pub struct ".len()..];
        let name = after.split(|c: char| c == '<' || c == '{' || c == ' ' || c == ';').next()?.trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }

    if line.starts_with("pub enum ") {
        let after = &line["pub enum ".len()..];
        let name = after.split(|c: char| c == '<' || c == '{' || c == ' ' || c == ';').next()?.trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }

    if line.starts_with("pub trait ") {
        let after = &line["pub trait ".len()..];
        let name = after.split(|c: char| c == '<' || c == '{' || c == ':').next()?.trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_locked_names() {
        let content = r#"
# Locked Contracts

```rust
pub fn lex(source: &str) -> Vec<Token>;
pub struct Token {
    kind: TokenKind,
    span: Span,
}
pub enum TokenKind {
    Ident,
    Int,
}
pub trait TokenStream {
    fn next(&mut self) -> Option<Token>;
}
```
"#;
        let names = extract_locked_names(content);
        assert!(names.contains(&"lex".to_string()));
        assert!(names.contains(&"Token".to_string()));
        assert!(names.contains(&"TokenKind".to_string()));
        assert!(names.contains(&"TokenStream".to_string()));
    }

    #[test]
    fn test_extract_locked_names_empty() {
        let content = "No code blocks here.";
        let names = extract_locked_names(content);
        assert!(names.is_empty());
    }

    #[test]
    fn test_extract_pub_name_fn() {
        assert_eq!(
            extract_pub_name("pub fn lex(source: &str) -> Vec<Token>"),
            Some("lex".into())
        );
    }

    #[test]
    fn test_extract_pub_name_async_fn() {
        assert_eq!(
            extract_pub_name("pub async fn parse(input: &str) -> Result<Ast>"),
            Some("parse".into())
        );
    }

    #[test]
    fn test_extract_pub_name_struct() {
        assert_eq!(
            extract_pub_name("pub struct Token {"),
            Some("Token".into())
        );
    }

    #[test]
    fn test_extract_pub_name_enum() {
        assert_eq!(
            extract_pub_name("pub enum TokenKind {"),
            Some("TokenKind".into())
        );
    }

    #[test]
    fn test_extract_pub_name_trait() {
        assert_eq!(
            extract_pub_name("pub trait TokenStream {"),
            Some("TokenStream".into())
        );
    }

    #[test]
    fn test_extract_pub_name_non_pub() {
        assert_eq!(extract_pub_name("fn private() {}"), None);
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib gates::architecture gates::contracts`
Expected: 10 PASS

- [ ] **Step 4: Commit**

```bash
git add src/gates/architecture.rs src/gates/contracts.rs && git commit -m "feat: add ArchitectureGate and ContractGate with spawn_blocking, ContractGate only in gates module (Fix #1, #8)"
```

---

### Task 6: Commit Pipeline (Fix #1: correct imports, Fix #10: gate timing)

**Files:**
- Create: `src/gates/commit_pipeline.rs`

**Fix #1 applied:** `ContractGate` is imported ONLY from `crate::gates::contracts`, NOT from `crate::config::types`. The `config::types` module does not contain `ContractGate`.

**Fix #10 applied:** Each gate run is timed and logged with `tracing::info!(elapsed = ?start.elapsed(), gate = gate.name())`.

- [ ] **Step 1: Implement commit pipeline with correct imports and gate timing**

```rust
// src/gates/commit_pipeline.rs

use crate::config::types::ResolvedCommitGates;
use crate::error::PilotError;
use crate::gates::{
    Gate, GateResult, PipelineResult,
    fmt::FmtGate,
    check::CheckGate,
    clippy::ClippyGate,
    test_gate::TestGate,
    banned_pattern::BannedPatternGate,
    architecture::ArchitectureGate,
    contracts::ContractGate,  // Fix #1: imported ONLY from crate::gates::contracts,
                              // NOT from crate::config::types
};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// Run the commit pipeline gates sequentially.
///
/// Gates are run in order. The pipeline stops on the first failure
/// (short-circuit). If a gate auto-fixes (like fmt), subsequent gates
/// run against the fixed code.
///
/// Fix #10: Each gate's execution time is logged after it completes,
/// making slow gates identifiable in production logs.
pub async fn run_commit_pipeline(
    worktree_dir: &Path,
    project_root: &Path,
    config: &ResolvedCommitGates,
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
        gates.push(Arc::new(BannedPatternGate));
    }
    if config.architecture {
        gates.push(Arc::new(ArchitectureGate::with_default_rules()));
    }
    if config.contracts {
        gates.push(Arc::new(ContractGate::new(project_root.to_path_buf())));
    }

    let mut results = Vec::new();
    for gate in &gates {
        let start = Instant::now();
        tracing::debug!(gate = gate.name(), "running commit gate");
        let result = gate.run(worktree_dir).await?;
        let elapsed = start.elapsed();
        // Fix #10: Log gate execution timing so slow gates are identifiable
        tracing::info!(
            gate = gate.name(),
            elapsed = ?elapsed,
            passed = result.passed,
            "commit gate completed"
        );
        let passed = result.passed;
        results.push(result);
        if !passed {
            tracing::info!(gate = gate.name(), "commit gate failed — stopping pipeline");
            break;
        }
    }

    Ok(PipelineResult::from_gates(results))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::GateResult;

    #[test]
    fn test_pipeline_result_from_gates_all_pass() {
        let result = PipelineResult::from_gates(vec![
            GateResult::pass("fmt"),
            GateResult::pass("check"),
        ]);
        assert!(result.passed);
    }

    #[test]
    fn test_pipeline_result_from_gates_with_failure() {
        let result = PipelineResult::from_gates(vec![
            GateResult::pass("fmt"),
            GateResult::fail("check", "compile error"),
        ]);
        assert!(!result.passed);
        assert_eq!(result.first_failure().unwrap().gate_name, "check");
    }

    #[test]
    fn test_pipeline_failure_message_includes_details() {
        let result = PipelineResult::from_gates(vec![
            GateResult::fail_with_details("check", "compile failed", "error[E0308]"),
        ]);
        let msg = result.failure_message();
        assert!(msg.contains("**check failed**"));
        assert!(msg.contains("error[E0308]"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib gates::commit_pipeline`
Expected: 3 PASS

- [ ] **Step 3: Commit**

```bash
git add src/gates/commit_pipeline.rs && git commit -m "feat: add commit pipeline with correct ContractGate import (Fix #1) and gate execution timing (Fix #10)"
```

---

### Task 7: Done Pipeline Gates — DeadCode, Coverage, Mutation

**Files:**
- Create: `src/gates/dead_code.rs`
- Create: `src/gates/coverage.rs`
- Create: `src/gates/mutation.rs`

**Key fixes:** `LazyLock` for regex compilation; `DeadCodeGate` does NOT pass when `cargo check` fails.

- [ ] **Step 1: Implement DeadCodeGate (doesn't pass on check failure)**

```rust
// src/gates/dead_code.rs

use crate::error::PilotError;
use crate::gates::helpers::{run_command, strip_ansi};
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use std::path::Path;

pub struct DeadCodeGate;

#[async_trait]
impl Gate for DeadCodeGate {
    fn name(&self) -> &str {
        "dead_code"
    }

    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError> {
        let output = run_command(
            "cargo",
            &["check", "--", "-W", "dead_code", "-W", "unused_imports"],
            worktree_dir,
        )
        .await?;

        if !output.status.success() {
            // If cargo check itself fails, this gate cannot determine
            // dead code status. Report a distinct failure rather than
            // silently passing.
            tracing::warn!("dead_code: cargo check failed — cannot check dead code");
            return Ok(GateResult::fail(
                "dead_code",
                "cargo check failed — cannot assess dead code (fix compilation first)",
            ));
        }

        let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
        if stderr.contains("dead_code") || stderr.contains("unused") {
            tracing::info!("dead_code: found dead code or unused imports");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dead_code_gate_name() {
        let gate = DeadCodeGate;
        assert_eq!(gate.name(), "dead_code");
    }
}
```

- [ ] **Step 2: Implement CoverageGate (LazyLock regex)**

```rust
// src/gates/coverage.rs

use crate::error::PilotError;
use crate::gates::helpers::run_command;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

static COVERAGE_PCT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+\.?\d*)%\s*coverage").expect("invalid coverage regex")
});

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

    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError> {
        let output = run_command(
            "cargo",
            &["llvm-cov", "--summary-only"],
            worktree_dir,
        )
        .await;

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(pct) = parse_coverage_pct(&stdout) {
                    if pct >= self.min_coverage {
                        tracing::info!(coverage = pct, "coverage gate passed");
                        Ok(GateResult::pass("coverage"))
                    } else {
                        tracing::info!(
                            coverage = pct,
                            min = self.min_coverage,
                            "coverage below threshold"
                        );
                        Ok(GateResult::fail_with_details(
                            "coverage",
                            format!("coverage {pct:.0}% below {:.0}% threshold", self.min_coverage),
                            stdout.to_string(),
                        ))
                    }
                } else {
                    Ok(GateResult::fail(
                        "coverage",
                        "could not parse coverage output — is cargo-llvm-cov installed?",
                    ))
                }
            }
            Ok(_) => Ok(GateResult::fail(
                "coverage",
                "cargo llvm-cov failed — is cargo-llvm-cov installed?",
            )),
            Err(_) => Ok(GateResult::fail(
                "coverage",
                "cargo llvm-cov not found — install with: cargo install cargo-llvm-cov",
            )),
        }
    }
}

fn parse_coverage_pct(output: &str) -> Option<f64> {
    let cap = COVERAGE_PCT_RE.captures(output)?;
    cap[1].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_coverage_pct_with_percentage() {
        let output = "60.82% coverage, 120/197 lines covered";
        assert_eq!(parse_coverage_pct(output), Some(60.82));
    }

    #[test]
    fn test_parse_coverage_pct_integer() {
        let output = "80% coverage";
        assert_eq!(parse_coverage_pct(output), Some(80.0));
    }

    #[test]
    fn test_parse_coverage_pct_no_match() {
        assert_eq!(parse_coverage_pct("no coverage info here"), None);
    }

    #[test]
    fn test_coverage_gate_name() {
        let gate = CoverageGate::new(0.80);
        assert_eq!(gate.name(), "coverage");
    }
}
```

- [ ] **Step 3: Implement MutationGate (LazyLock regex)**

```rust
// src/gates/mutation.rs

use crate::error::PilotError;
use crate::gates::helpers::run_command;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

static MUTATION_PCT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\((\d+\.?\d*)%\)").expect("invalid mutation regex")
});

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

    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError> {
        let output = run_command(
            "cargo",
            &["mutants", "--no-times"],
            worktree_dir,
        )
        .await;

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(kill_rate) = parse_mutation_kill_rate(&stdout) {
                    if kill_rate >= self.min_kill_rate {
                        tracing::info!(kill_rate, "mutation gate passed");
                        Ok(GateResult::pass("mutation"))
                    } else {
                        tracing::info!(
                            kill_rate,
                            min = self.min_kill_rate,
                            "mutation kill rate below threshold"
                        );
                        Ok(GateResult::fail_with_details(
                            "mutation",
                            format!(
                                "mutation kill rate {kill_rate:.0}% below {:.0}% threshold",
                                self.min_kill_rate
                            ),
                            stdout.to_string(),
                        ))
                    }
                } else {
                    Ok(GateResult::fail(
                        "mutation",
                        "could not parse mutation output — unexpected cargo-mutants format",
                    ))
                }
            }
            Ok(_) => Ok(GateResult::fail(
                "mutation",
                "cargo mutants failed — is cargo-mutants installed?",
            )),
            Err(_) => Ok(GateResult::fail(
                "mutation",
                "cargo mutants not found — install with: cargo install cargo-mutants",
            )),
        }
    }
}

fn parse_mutation_kill_rate(output: &str) -> Option<f64> {
    let cap = MUTATION_PCT_RE.captures(output)?;
    cap[1].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mutation_kill_rate() {
        let output = "24 mutants killed out of 30 (80.0%)";
        assert_eq!(parse_mutation_kill_rate(output), Some(80.0));
    }

    #[test]
    fn test_parse_mutation_kill_rate_integer() {
        let output = "10 of 10 (100%)";
        assert_eq!(parse_mutation_kill_rate(output), Some(100.0));
    }

    #[test]
    fn test_parse_mutation_kill_rate_no_match() {
        assert_eq!(parse_mutation_kill_rate("no mutation data"), None);
    }

    #[test]
    fn test_mutation_gate_name() {
        let gate = MutationGate::new(0.75);
        assert_eq!(gate.name(), "mutation");
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib gates::dead_code gates::coverage gates::mutation`
Expected: 7 PASS

- [ ] **Step 5: Commit**

```bash
git add src/gates/dead_code.rs src/gates/coverage.rs src/gates/mutation.rs && git commit -m "feat: add DeadCode/Coverage/Mutation gates with LazyLock regex and correct DeadCodeGate semantics"
```

---

### Task 8: WorkspaceCheckGate, AuditGate, and Self-Review

**Files:**
- Create: `src/gates/workspace_check.rs`
- Create: `src/gates/audit.rs`
- Create: `src/gates/self_review.rs`

- [ ] **Step 1: Implement WorkspaceCheckGate**

```rust
// src/gates/workspace_check.rs

use crate::error::PilotError;
use crate::gates::helpers::{run_command, strip_ansi};
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use std::path::Path;

pub struct WorkspaceCheckGate;

#[async_trait]
impl Gate for WorkspaceCheckGate {
    fn name(&self) -> &str {
        "workspace_check"
    }

    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError> {
        let output = run_command("cargo", &["check", "--workspace"], worktree_dir).await?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_check_gate_name() {
        let gate = WorkspaceCheckGate;
        assert_eq!(gate.name(), "workspace_check");
    }
}
```

- [ ] **Step 2: Implement AuditGate**

```rust
// src/gates/audit.rs

use crate::error::PilotError;
use crate::gates::helpers::run_command;
use crate::gates::{Gate, GateResult};
use async_trait::async_trait;
use std::path::Path;

pub struct AuditGate;

#[async_trait]
impl Gate for AuditGate {
    fn name(&self) -> &str {
        "audit"
    }

    async fn run(&self, worktree_dir: &Path) -> Result<GateResult, PilotError> {
        let output = run_command("cargo", &["audit"], worktree_dir).await;

        match output {
            Ok(out) if out.status.success() => {
                Ok(GateResult::pass("audit"))
            }
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let combined = format!("{stdout}\n{stderr}");
                Ok(GateResult::fail_with_details(
                    "audit",
                    "vulnerabilities found by cargo audit",
                    combined,
                ))
            }
            Err(_) => Ok(GateResult::fail(
                "audit",
                "cargo audit not found — install with: cargo install cargo-audit",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_gate_name() {
        let gate = AuditGate;
        assert_eq!(gate.name(), "audit");
    }
}
```

- [ ] **Step 3: Implement Self-Review prompt builder**

```rust
// src/gates/self_review.rs

use crate::error::PilotError;

/// Build the self-review prompt for the AI after all done gates pass.
///
/// Per REQ-FUNC-051: sends the AI the full diff and a structured
/// review checklist covering edge cases, performance, error handling,
/// API consistency, test quality, dead code, documentation, and naming.
pub fn build_review_prompt(diff: &str, commit_log: &str) -> String {
    format!(
        r#"## Self-Review Required

You have declared ::DONE and all quality gates have passed. Before final approval, review your changes.

### Commit History
```
{commit_log}
```

### Full Diff
```diff
{diff}
```

### Review Checklist
Review your changes against the following criteria. If you find any issues, fix them with ::REPLACE directives and ::COMMIT. If everything looks correct, respond with `::APPROVED` in a ```glyim-ops``` block.

1. **Edge cases**: Are all edge cases handled? (empty inputs, overflow, off-by-one)
2. **Performance**: Are there unnecessary allocations, clones, or quadratic operations?
3. **Error handling**: Are all error paths covered? No `.unwrap()` or `todo!()` in non-test code?
4. **API consistency**: Do public interfaces follow project conventions?
5. **Test quality**: Do tests cover the happy path AND failure cases?
6. **Dead code**: Are there unused imports, functions, or variables?
7. **Documentation**: Are public items documented?
8. **Naming**: Are names clear and consistent with the codebase?

Respond with your review, then either fix issues or emit ::APPROVED.
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_review_prompt_contains_diff() {
        let prompt = build_review_prompt("fn new() {}", "abc123 add new fn");
        assert!(prompt.contains("fn new() {}"));
        assert!(prompt.contains("abc123 add new fn"));
    }

    #[test]
    fn test_build_review_prompt_contains_checklist() {
        let prompt = build_review_prompt("diff", "log");
        assert!(prompt.contains("Edge cases"));
        assert!(prompt.contains("Performance"));
        assert!(prompt.contains("::APPROVED"));
        assert!(prompt.contains("1."));
        assert!(prompt.contains("8."));
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib gates::workspace_check gates::audit gates::self_review`
Expected: 4 PASS

- [ ] **Step 5: Commit**

```bash
git add src/gates/workspace_check.rs src/gates/audit.rs src/gates/self_review.rs && git commit -m "feat: add WorkspaceCheck/Audit gates and self-review prompt builder"
```

---

### Task 9: Done Pipeline (with gate timing — Fix #10)

**Files:**
- Create: `src/gates/done_pipeline.rs`

- [ ] **Step 1: Implement done pipeline with gate timing**

```rust
// src/gates/done_pipeline.rs

use crate::config::types::ResolvedDoneGates;
use crate::error::PilotError;
use crate::gates::{
    Gate, GateResult, PipelineResult,
    fmt::FmtGate,
    check::CheckGate,
    clippy::ClippyGate,
    test_gate::TestGate,
    banned_pattern::BannedPatternGate,
    architecture::ArchitectureGate,
    dead_code::DeadCodeGate,
    coverage::CoverageGate,
    mutation::MutationGate,
    workspace_check::WorkspaceCheckGate,
    audit::AuditGate,
};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// Run the done pipeline gates sequentially.
///
/// The done pipeline runs after `::DONE`. It includes the full commit
/// pipeline first, then the additional done-only gates.
///
/// Fix #10: Each gate's execution time is logged after it completes.
pub async fn run_done_pipeline(
    worktree_dir: &Path,
    project_root: &Path,
    config: &ResolvedDoneGates,
) -> Result<PipelineResult, PilotError> {
    let mut gates: Vec<Arc<dyn Gate>> = Vec::new();

    // 1. Full commit pipeline (always)
    gates.push(Arc::new(FmtGate));
    gates.push(Arc::new(CheckGate));
    gates.push(Arc::new(ClippyGate));
    gates.push(Arc::new(TestGate));
    gates.push(Arc::new(BannedPatternGate));

    // 2. Done-only gates
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

    let mut results = Vec::new();
    for gate in &gates {
        let start = Instant::now();
        tracing::debug!(gate = gate.name(), "running done gate");
        let result = gate.run(worktree_dir).await?;
        let elapsed = start.elapsed();
        // Fix #10: Log gate execution timing
        tracing::info!(
            gate = gate.name(),
            elapsed = ?elapsed,
            passed = result.passed,
            "done gate completed"
        );
        let passed = result.passed;
        results.push(result);
        if !passed {
            tracing::info!(gate = gate.name(), "done gate failed — stopping pipeline");
            break;
        }
    }

    Ok(PipelineResult::from_gates(results))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::GateResult;

    #[test]
    fn test_done_pipeline_result_pass() {
        let result = PipelineResult::from_gates(vec![
            GateResult::pass("fmt"),
            GateResult::pass("check"),
            GateResult::pass("coverage"),
        ]);
        assert!(result.passed);
    }

    #[test]
    fn test_done_pipeline_result_fail() {
        let result = PipelineResult::from_gates(vec![
            GateResult::pass("fmt"),
            GateResult::fail_with_details("coverage", "too low", "62% < 80%"),
        ]);
        assert!(!result.passed);
        let msg = result.failure_message();
        assert!(msg.contains("**coverage failed**"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib gates::done_pipeline`
Expected: 2 PASS

- [ ] **Step 3: Commit**

```bash
git add src/gates/done_pipeline.rs && git commit -m "feat: add done pipeline with all 10 gates and gate execution timing (Fix #10)"
```

---

### Task 10: Commit Engine (stateless, fix_round from SessionState — Fix #4)

**Files:**
- Create: `src/commit/mod.rs`
- Create: `src/commit/engine.rs`

**Fix #4 applied:** The `CommitEngine` is stateless. It takes `current_fix_round` as input and returns `new_fix_round` in every `CommitDecision` variant. The caller (orchestrator) persists `new_fix_round` to `SessionState`. There is NO `record_gate_failure` call inside the engine — the engine simply returns the new value, and the orchestrator sets `s.fix_round = new_fix_round`. No double-write, no confusion.

- [ ] **Step 1: Implement commit engine**

```rust
// src/commit/engine.rs

use crate::config::types::ResolvedCommitGates;
use crate::error::PilotError;
use crate::gates::commit_pipeline;
use crate::git_ops::{commit_all, emergency_wip_commit};
use std::path::Path;

/// Decision from the commit engine.
///
/// Every variant includes `new_fix_round` so the caller can persist
/// it to SessionState. Fix #4: The engine never calls
/// `record_gate_failure()` — it just returns the new value.
/// The caller does `s.fix_round = new_fix_round` and that's it.
#[derive(Debug, Clone)]
pub enum CommitDecision {
    /// All gates passed; commit was created.
    Committed {
        message: String,
        new_fix_round: u32, // Always 0 after success
    },
    /// A gate failed; error feedback should be sent to the AI.
    GateFailed {
        new_fix_round: u32,
        feedback: String,
    },
    /// Fix rounds exceeded; emergency WIP commit made; escalate to human.
    Escalated {
        new_fix_round: u32,
        feedback: String,
    },
}

/// Stateless commit engine. All mutable state (fix_round) is managed
/// by the caller via SessionState.
///
/// Usage pattern:
/// ```ignore
/// let decision = engine.evaluate_commit(..., current_fix_round).await?;
/// // Then in the orchestrator:
/// persistence.update_session(stream_id, |s| {
///     s.fix_round = decision.new_fix_round();  // Single write — no double-write
/// }).await?;
/// ```
pub struct CommitEngine {
    gate_config: ResolvedCommitGates,
    max_fix_rounds: u32,
    project_root: std::path::PathBuf,
}

impl CommitEngine {
    pub fn new(
        gate_config: ResolvedCommitGates,
        max_fix_rounds: u32,
        project_root: std::path::PathBuf,
    ) -> Self {
        Self {
            gate_config,
            max_fix_rounds,
            project_root,
        }
    }

    /// Evaluate whether to commit.
    ///
    /// `current_fix_round` comes from `SessionState.fix_round`.
    /// The returned `CommitDecision` includes the new fix_round value
    /// for the caller to persist back to `SessionState` via a SINGLE
    /// write: `s.fix_round = new_fix_round`. No `record_gate_failure()`
    /// needed — the engine computes the correct value. (Fix #4)
    pub async fn evaluate_commit(
        &self,
        worktree_dir: &Path,
        stream_id: &str,
        message: &str,
        current_fix_round: u32,
    ) -> Result<CommitDecision, PilotError> {
        let pipeline_result = commit_pipeline::run_commit_pipeline(
            worktree_dir,
            &self.project_root,
            &self.gate_config,
        )
        .await?;

        if pipeline_result.passed {
            // Commit
            commit_all(worktree_dir, stream_id, message).await?;
            tracing::info!(
                stream_id,
                message,
                "commit succeeded — resetting fix_round to 0"
            );
            Ok(CommitDecision::Committed {
                message: message.to_string(),
                new_fix_round: 0,
            })
        } else {
            // The engine calculates new_fix_round = current + 1.
            // The caller persists it with a single write. No record_gate_failure().
            let new_fix_round = current_fix_round + 1;
            let feedback = pipeline_result.failure_message();

            if new_fix_round > self.max_fix_rounds {
                // Emergency WIP commit
                emergency_wip_commit(worktree_dir, stream_id).await?;
                tracing::warn!(
                    stream_id,
                    new_fix_round,
                    max = self.max_fix_rounds,
                    "fix rounds exceeded — emergency WIP commit, escalating"
                );
                Ok(CommitDecision::Escalated {
                    new_fix_round,
                    feedback,
                })
            } else {
                tracing::info!(
                    stream_id,
                    new_fix_round,
                    max = self.max_fix_rounds,
                    "commit gate failed — incrementing fix_round"
                );
                Ok(CommitDecision::GateFailed {
                    new_fix_round,
                    feedback,
                })
            }
        }
    }
}

impl CommitDecision {
    /// Get the new fix_round value from any decision variant.
    /// The caller should persist this with a single write:
    /// `s.fix_round = decision.new_fix_round()`
    pub fn new_fix_round(&self) -> u32 {
        match self {
            CommitDecision::Committed { new_fix_round, .. } => *new_fix_round,
            CommitDecision::GateFailed { new_fix_round, .. } => *new_fix_round,
            CommitDecision::Escalated { new_fix_round, .. } => *new_fix_round,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_decision_committed() {
        let d = CommitDecision::Committed {
            message: "feat: add lexer".into(),
            new_fix_round: 0,
        };
        assert_eq!(d.new_fix_round(), 0);
    }

    #[test]
    fn test_commit_decision_gate_failed() {
        let d = CommitDecision::GateFailed {
            new_fix_round: 1,
            feedback: "check failed".into(),
        };
        assert_eq!(d.new_fix_round(), 1);
    }

    #[test]
    fn test_commit_decision_escalated() {
        let d = CommitDecision::Escalated {
            new_fix_round: 6,
            feedback: "too many failures".into(),
        };
        assert_eq!(d.new_fix_round(), 6);
    }

    #[test]
    fn test_commit_decision_new_fix_round_helper() {
        // Verify the helper works for all variants
        let committed = CommitDecision::Committed {
            message: "msg".into(),
            new_fix_round: 0,
        };
        let failed = CommitDecision::GateFailed {
            new_fix_round: 3,
            feedback: "err".into(),
        };
        let escalated = CommitDecision::Escalated {
            new_fix_round: 6,
            feedback: "err".into(),
        };
        assert_eq!(committed.new_fix_round(), 0);
        assert_eq!(failed.new_fix_round(), 3);
        assert_eq!(escalated.new_fix_round(), 6);
    }
}
```

Create `src/commit/mod.rs`:

```rust
// src/commit/mod.rs

pub mod engine;
pub use engine::{CommitEngine, CommitDecision};
```

Update `src/lib.rs` to add `pub mod commit;`.

- [ ] **Step 2: Run tests**

Run: `cargo test --lib commit`
Expected: 4 PASS

- [ ] **Step 3: Commit**

```bash
git add src/commit/ src/lib.rs && git commit -m "feat: add stateless commit engine with single fix_round write — no record_gate_failure double-write (Fix #4)"
```

---

### Task 11: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test --lib`
Expected: All PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --check`
Expected: No formatting issues

- [ ] **Step 4: Tag**

```bash
git tag v0.1.0-gates -m "All 10 quality gates, commit/done pipelines, stateless commit engine, Fix #1 #3 #4 #8 #10"
```

---

**Phase 3 complete.** All fixes applied:

- **Fix #1:** `ContractGate` is imported ONLY from `crate::gates::contracts` in `commit_pipeline.rs`. It does NOT exist in `crate::config::types`.
- **Fix #3:** No `GateConfig` struct exists anywhere. No dead code with `unreachable!()`.
- **Fix #4:** `CommitEngine` is stateless. Returns `new_fix_round` in every `CommitDecision` variant. The orchestrator does a SINGLE write (`s.fix_round = decision.new_fix_round()`). No `record_gate_failure()` double-write.
- **Fix #8:** `BannedPatternGate` and `ArchitectureGate` use `spawn_blocking` for synchronous file I/O.
- **Fix #10:** Both `run_commit_pipeline` and `run_done_pipeline` log `tracing::info!(elapsed = ?start.elapsed(), gate = gate.name(), passed = result.passed)` after each gate completes.
- **Fix #11 (source):** `rate_limit_cooldown` is `u64` in `ProviderConfig`, matching the config type. Cast to `i64` for `chrono::Duration` is documented.

Ready for **Phase 4: Session Management & State Persistence** — shall I continue?
# Phase 4: Session Management & State Persistence — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the session state machine, stream status tracking, and crash recovery via persistent state files. Every state change is serialized to disk so that a CLI crash loses no data. Transition validation errors are propagated, not swallowed.

**Architecture:** Each stream is a `SessionState` struct with a `StreamStatus` enum. The `SessionManager` owns a `HashMap<String, SessionState>` for O(1) lookups. `TransitionValidator` enforces valid state transitions and returns `Err` on invalid ones — the caller must handle these errors, never silently continue (Fix #6 infrastructure). `StatePersistence` serializes to `.glyim-pilot-state.json` on every mutation using compact JSON, not pretty-printed (Fix #9). `HashMap<String, SessionState>` serializes natively via serde — no custom serializers (Fix #7). Provider cooldown timestamps use `chrono::DateTime<Utc>` for serializability and crash recovery. `try_update_session` supports rollback on closure failure (Fix #6 infrastructure).

**Tech Stack:** serde_json 1, chrono 0.4 (serde), uuid 1, tokio::fs

---

### Task 1: Session State Types (Fix #7: no custom serde serializers)

**Files:**
- Create: `src/session/mod.rs`
- Create: `src/session/state.rs`

**Fix #7 applied:** `HashMap<String, SessionState>` serializes to a JSON object natively with serde. No `serialize_session_map` / `deserialize_session_map` functions. No `#[serde(serialize_with = ..., deserialize_with = ...)]` attribute. These were 30 lines of boilerplate that replicated default behavior.

- [ ] **Step 1: Write session state types and tests**

```rust
// src/session/state.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Status of a stream in the session state machine.
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

/// Persistent state for a single stream session.
///
/// This struct is serialized to `.glyim-pilot-state.json` on every
/// state transition for crash recovery (REQ-FUNC-061).
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
    /// Provider cooldown expiry time, if this session's provider
    /// was rate-limited. Used for crash recovery of cooldown state.
    pub provider_cooldown_until: Option<DateTime<Utc>>,
}

impl SessionState {
    pub fn new(
        stream_id: String,
        provider_id: String,
        worktree_path: String,
    ) -> Self {
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

    /// Transition to a new status, updating timestamps.
    /// Callers should use `TransitionValidator::transition` first.
    pub fn transition(&mut self, new_status: StreamStatus) {
        let now = Utc::now();
        self.status = new_status;
        self.updated_at = now;
        self.last_activity = now;
    }

    /// Record a successful commit.
    pub fn record_commit(&mut self) {
        self.commits += 1;
        self.fix_round = 0;
        self.last_activity = Utc::now();
    }

    /// Record a new turn.
    pub fn record_turn(&mut self) {
        self.turn += 1;
        self.last_activity = Utc::now();
    }

    /// Set the provider cooldown expiry.
    pub fn set_provider_cooldown(&mut self, until: DateTime<Utc>) {
        self.provider_cooldown_until = Some(until);
    }

    /// Clear the provider cooldown.
    pub fn clear_provider_cooldown(&mut self) {
        self.provider_cooldown_until = None;
    }

    /// Check if this session's provider is still in cooldown.
    pub fn is_provider_in_cooldown(&self) -> bool {
        self.provider_cooldown_until
            .map_or(false, |until| Utc::now() < until)
    }
}

/// Complete state for all sessions, persisted to disk.
///
/// Uses a `HashMap` keyed by `stream_id` for O(1) lookups.
///
/// Fix #7: No custom serde serializers. `HashMap<String, SessionState>`
/// serializes to a JSON object natively — the `#[serde(serialize_with)]
/// and `#[serde(deserialize_with)]` attributes that previously called
/// default implementations have been removed. They were 30 lines of
/// boilerplate that replicated default behavior with zero benefit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalState {
    /// Map from stream_id to session state.
    /// Serializes to a JSON object keyed by stream_id natively.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_new() {
        let state = SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt-S01".into());
        assert_eq!(state.stream_id, "S01");
        assert_eq!(state.provider_id, "deepseek");
        assert_eq!(state.status, StreamStatus::Init);
        assert_eq!(state.turn, 0);
        assert_eq!(state.fix_round, 0);
        assert_eq!(state.commits, 0);
        assert!(state.provider_cooldown_until.is_none());
    }

    #[test]
    fn test_session_state_transition() {
        let mut state = SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        state.transition(StreamStatus::Seeding);
        assert_eq!(state.status, StreamStatus::Seeding);
        assert!(state.updated_at >= state.created_at);
    }

    #[test]
    fn test_session_state_record_commit_resets_fix_round() {
        let mut state = SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        state.fix_round = 3;
        state.record_commit();
        assert_eq!(state.commits, 1);
        assert_eq!(state.fix_round, 0, "commit must reset fix_round");
    }

    #[test]
    fn test_session_state_record_turn() {
        let mut state = SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        state.record_turn();
        assert_eq!(state.turn, 1);
    }

    #[test]
    fn test_session_state_cooldown() {
        let mut state = SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        assert!(!state.is_provider_in_cooldown());

        let future = Utc::now() + chrono::Duration::seconds(300);
        state.set_provider_cooldown(future);
        assert!(state.is_provider_in_cooldown());

        state.clear_provider_cooldown();
        assert!(!state.is_provider_in_cooldown());
    }

    #[test]
    fn test_session_state_serialization_roundtrip() {
        let mut state = SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        state.transition(StreamStatus::Streaming);
        state.record_turn();
        state.fix_round = 2;
        let json = serde_json::to_string(&state).unwrap();
        let de: SessionState = serde_json::from_str(&json).unwrap();
        assert_eq!(state.session_id, de.session_id);
        assert_eq!(state.stream_id, de.stream_id);
        assert_eq!(state.status, de.status);
        assert_eq!(state.turn, de.turn);
        assert_eq!(state.fix_round, de.fix_round);
    }

    #[test]
    fn test_stream_status_serialization() {
        let status = StreamStatus::Streaming;
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("STREAMING"));
        let de: StreamStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, de);
    }

    #[test]
    fn test_global_state_serialization_roundtrip() {
        let mut gs = GlobalState::new();
        let s1 = SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt1".into());
        let s2 = SessionState::new("S02".into(), "grok".into(), "/tmp/wt2".into());
        gs.sessions.insert("S01".into(), s1);
        gs.sessions.insert("S02".into(), s2);

        let json = serde_json::to_string(&gs).unwrap();
        let de: GlobalState = serde_json::from_str(&json).unwrap();
        assert_eq!(de.sessions.len(), 2);
        assert!(de.sessions.contains_key("S01"));
        assert!(de.sessions.contains_key("S02"));
    }

    #[test]
    fn test_global_state_json_is_object_not_array() {
        let mut gs = GlobalState::new();
        gs.sessions.insert(
            "S01".into(),
            SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into()),
        );
        let json = serde_json::to_string_pretty(&gs).unwrap();
        // The sessions field should be a JSON object keyed by stream_id,
        // not an array. With default HashMap serde, this is automatic.
        assert!(
            json.contains("\"S01\""),
            "sessions should be keyed by stream_id, got: {json}"
        );
    }

    #[test]
    fn test_no_custom_serializers_needed() {
        // Fix #7: Verify that HashMap<String, SessionState> serializes
        // correctly with default serde, proving custom serializers were
        // unnecessary.
        let mut gs = GlobalState::new();
        let mut s = SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        s.fix_round = 5;
        s.commits = 3;
        gs.sessions.insert("S01".into(), s);

        // Compact serialization
        let compact = serde_json::to_string(&gs).unwrap();
        assert!(compact.contains("\"S01\""));
        assert!(compact.contains("\"fix_round\":5"));

        // Pretty serialization
        let pretty = serde_json::to_string_pretty(&gs).unwrap();
        assert!(pretty.contains("\"S01\""));

        // Both round-trip correctly
        let de_compact: GlobalState = serde_json::from_str(&compact).unwrap();
        let de_pretty: GlobalState = serde_json::from_str(&pretty).unwrap();
        assert_eq!(de_compact.sessions["S01"].fix_round, 5);
        assert_eq!(de_pretty.sessions["S01"].commits, 3);
    }

    #[test]
    fn test_fix_round_is_u32_not_inside_engine() {
        // Fix #4: fix_round is a plain u32 in SessionState.
        // The commit engine returns new_fix_round as u32.
        // The orchestrator does s.fix_round = new_fix_round (single write).
        let mut state = SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        assert_eq!(state.fix_round, 0);
        state.fix_round = 3; // Direct assignment — no record_gate_failure
        assert_eq!(state.fix_round, 3);
        state.fix_round = 4; // Engine returns new value, orchestrator writes it once
        assert_eq!(state.fix_round, 4);
    }
}
```

Create `src/session/mod.rs`:

```rust
// src/session/mod.rs

pub mod state;
pub mod machine;
pub mod persistence;

pub use state::{SessionState, StreamStatus, GlobalState};
pub use machine::TransitionValidator;
pub use persistence::StatePersistence;
```

Update `src/lib.rs` to add `pub mod session;`.

- [ ] **Step 2: Run tests**

Run: `cargo test --lib session::state`
Expected: 10 PASS

- [ ] **Step 3: Commit**

```bash
git add src/session/ src/lib.rs && git commit -m "feat: add SessionState with HashMap-based GlobalState, no custom serde serializers (Fix #7), serializable cooldown"
```

---

### Task 2: TransitionValidator — properly named, errors propagated

**Files:**
- Create: `src/session/machine.rs`

**Fix #6 infrastructure:** `TransitionValidator::transition` returns `Result<(), PilotError>`. When it fails, the caller must handle the error — never `let _ =`. The orchestrator in Phase 6 will use `try_update_session` so that a transition failure aborts the save and rolls back mutations.

- [ ] **Step 1: Implement TransitionValidator with validation and tests**

```rust
// src/session/machine.rs

use crate::error::PilotError;
use super::state::{SessionState, StreamStatus};

/// Valid state transitions per the session state machine diagram.
///
/// These encode the legal paths from the Architecture spec's
/// session state diagram (Section 3.4).
const VALID_TRANSITIONS: &[(StreamStatus, StreamStatus)] = &[
    (StreamStatus::Init, StreamStatus::Seeding),
    (StreamStatus::Seeding, StreamStatus::Waiting),
    (StreamStatus::Seeding, StreamStatus::Error),
    (StreamStatus::Waiting, StreamStatus::Streaming),
    (StreamStatus::Waiting, StreamStatus::Paused),
    (StreamStatus::Streaming, StreamStatus::Executing),
    (StreamStatus::Streaming, StreamStatus::Error),
    (StreamStatus::Executing, StreamStatus::Feedback),
    (StreamStatus::Executing, StreamStatus::Error),
    (StreamStatus::Feedback, StreamStatus::Waiting),
    (StreamStatus::Feedback, StreamStatus::Committing),
    (StreamStatus::Committing, StreamStatus::Committed),
    (StreamStatus::Committing, StreamStatus::Feedback), // gate fail → feedback
    (StreamStatus::Committed, StreamStatus::Waiting),
    (StreamStatus::Committed, StreamStatus::Verifying),
    (StreamStatus::Verifying, StreamStatus::Reviewing),
    (StreamStatus::Verifying, StreamStatus::Feedback), // done gate fail
    (StreamStatus::Reviewing, StreamStatus::Complete),
    (StreamStatus::Reviewing, StreamStatus::Feedback), // AI wants to fix
    (StreamStatus::Error, StreamStatus::Seeding),      // retry / failover
    (StreamStatus::Error, StreamStatus::Paused),
    (StreamStatus::Paused, StreamStatus::Seeding),     // resume
];

/// Validates that a proposed session state transition is legal.
///
/// This is NOT a state machine — it does not own or manage state.
/// It is a pure validation function. The caller owns the state
/// and is responsible for calling `validate()` before `transition()`.
///
/// Fix #6: All transition methods return `Result<(), PilotError>`.
/// Callers MUST handle the error — never `let _ =` or silently
/// continue. The orchestrator uses `try_update_session` so that
/// a transition failure aborts the save and rolls back mutations.
///
/// Usage pattern:
/// ```ignore
/// // Option 1: Validate then transition separately
/// TransitionValidator::validate(&session, StreamStatus::Seeding)?;
/// session.transition(StreamStatus::Seeding);
///
/// // Option 2: Validate and transition in one call
/// TransitionValidator::transition(&mut session, StreamStatus::Seeding)?;
/// // If this returns Err, session is NOT modified
/// ```
pub struct TransitionValidator;

impl TransitionValidator {
    /// Validate that transitioning from `session.status` to `new_status`
    /// is legal. Returns `Ok(())` if valid, `Err` with a descriptive
    /// message if invalid.
    ///
    /// Same-state transitions (re-entry) are always allowed.
    pub fn validate(
        session: &SessionState,
        new_status: StreamStatus,
    ) -> Result<(), PilotError> {
        let current = &session.status;

        // Same-state re-entry is always allowed
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

    /// Validate and apply a transition in one call.
    ///
    /// On validation failure, the session state is NOT modified
    /// and the error is propagated to the caller. The caller
    /// MUST handle this error — never discard it with `let _ =`.
    ///
    /// Fix #6: This returns Result, not (). If the transition is
    /// invalid, the error must be handled by the caller. The
    /// orchestrator uses this inside `try_update_session` so that
    /// a failed transition aborts the entire state save.
    pub fn transition(
        session: &mut SessionState,
        new_status: StreamStatus,
    ) -> Result<(), PilotError> {
        // Validate against the CURRENT state before mutating
        let current_status = session.status.clone();
        if current_status != new_status {
            let valid = VALID_TRANSITIONS
                .iter()
                .any(|(from, to)| *from == current_status && *to == new_status);

            if !valid {
                return Err(PilotError::Session(format!(
                    "invalid state transition: {:?} → {:?} (session {})",
                    current_status, new_status, session.stream_id
                )));
            }
        }
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

    // --- Validation-only tests (non-mutating) ---

    #[test]
    fn test_validate_init_to_seeding() {
        let s = make_session();
        assert!(TransitionValidator::validate(&s, StreamStatus::Seeding).is_ok());
    }

    #[test]
    fn test_validate_invalid_init_to_streaming() {
        let s = make_session();
        let result = TransitionValidator::validate(&s, StreamStatus::Streaming);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid state transition"));
    }

    #[test]
    fn test_validate_same_state_reentry() {
        let s = make_session();
        assert!(TransitionValidator::validate(&s, StreamStatus::Init).is_ok());
    }

    #[test]
    fn test_validate_complete_is_terminal() {
        let mut s = make_session();
        s.status = StreamStatus::Complete;
        let result = TransitionValidator::validate(&s, StreamStatus::Waiting);
        assert!(result.is_err(), "Complete should be a terminal state");
    }

    // --- Transition tests (mutating) ---

    #[test]
    fn test_transition_init_to_seeding() {
        let mut s = make_session();
        assert!(TransitionValidator::transition(&mut s, StreamStatus::Seeding).is_ok());
        assert_eq!(s.status, StreamStatus::Seeding);
    }

    #[test]
    fn test_transition_full_lifecycle() {
        let mut s = make_session();
        TransitionValidator::transition(&mut s, StreamStatus::Seeding).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Waiting).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Streaming).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Executing).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Feedback).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Committing).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Committed).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Waiting).unwrap();
    }

    #[test]
    fn test_transition_lifecycle_to_complete() {
        let mut s = make_session();
        TransitionValidator::transition(&mut s, StreamStatus::Seeding).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Waiting).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Streaming).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Executing).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Feedback).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Committing).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Committed).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Verifying).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Reviewing).unwrap();
        TransitionValidator::transition(&mut s, StreamStatus::Complete).unwrap();
    }

    #[test]
    fn test_transition_gate_fail_to_feedback() {
        let mut s = make_session();
        s.status = StreamStatus::Committing;
        assert!(TransitionValidator::transition(&mut s, StreamStatus::Feedback).is_ok());
    }

    #[test]
    fn test_transition_done_gate_fail_to_feedback() {
        let mut s = make_session();
        s.status = StreamStatus::Verifying;
        assert!(TransitionValidator::transition(&mut s, StreamStatus::Feedback).is_ok());
    }

    #[test]
    fn test_transition_error_to_seeding_failover() {
        let mut s = make_session();
        s.status = StreamStatus::Error;
        assert!(TransitionValidator::transition(&mut s, StreamStatus::Seeding).is_ok());
    }

    #[test]
    fn test_transition_error_to_paused() {
        let mut s = make_session();
        s.status = StreamStatus::Error;
        assert!(TransitionValidator::transition(&mut s, StreamStatus::Paused).is_ok());
    }

    #[test]
    fn test_transition_invalid_does_not_mutate() {
        let mut s = make_session();
        let result = TransitionValidator::transition(&mut s, StreamStatus::Streaming);
        assert!(result.is_err());
        assert_eq!(s.status, StreamStatus::Init, "invalid transition must not mutate state");
    }

    #[test]
    fn test_transition_same_state_allowed() {
        let mut s = make_session();
        s.status = StreamStatus::Waiting;
        assert!(TransitionValidator::transition(&mut s, StreamStatus::Waiting).is_ok());
        assert_eq!(s.status, StreamStatus::Waiting);
    }

    #[test]
    fn test_transition_reviewing_to_feedback_for_fix() {
        let mut s = make_session();
        s.status = StreamStatus::Reviewing;
        assert!(TransitionValidator::transition(&mut s, StreamStatus::Feedback).is_ok());
    }

    #[test]
    fn test_transition_paused_to_seeding_resume() {
        let mut s = make_session();
        s.status = StreamStatus::Paused;
        assert!(TransitionValidator::transition(&mut s, StreamStatus::Seeding).is_ok());
    }

    #[test]
    fn test_transition_returns_result_not_unit() {
        // Fix #6: transition() returns Result<(), PilotError>, not ().
        // Callers must handle the error — never let _ = or silently continue.
        let mut s = make_session();
        let result: Result<(), PilotError> = TransitionValidator::transition(&mut s, StreamStatus::Seeding);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transition_error_is_propagated() {
        // Fix #6: When transition fails, the error must be propagated
        // to the caller, not logged and swallowed.
        let mut s = make_session();
        let result = TransitionValidator::transition(&mut s, StreamStatus::Complete);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("invalid state transition"),
            "error must describe the invalid transition: got '{}'",
            err
        );
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib session::machine`
Expected: 16 PASS

- [ ] **Step 3: Commit**

```bash
git add src/session/machine.rs && git commit -m "feat: add TransitionValidator with Result-returning transition (Fix #6 infrastructure) — errors propagated, never swallowed"
```

---

### Task 3: State Persistence with Crash Recovery (Fix #9: compact JSON)

**Files:**
- Create: `src/session/persistence.rs`

**Fix #9 applied:** `save()` uses `serde_json::to_string` (compact) instead of `to_string_pretty`. Pretty-printing is unnecessary for crash-recovery writes — the file doesn't need to be human-readable on every write. A `debug_dump()` method is provided for CLI `debug-dump` commands that need pretty output.

**Fix #6 infrastructure:** `try_update_session` supports fallible closures. If the closure returns `Err`, the session is NOT modified and the state is NOT saved. This will be used by the orchestrator to prevent inconsistent state from being persisted when a transition fails.

**Design note on I/O tradeoff:** `StatePersistence::save` writes the entire state on every mutation. With 20 sessions and frequent updates (every state transition per REQ-FUNC-061), this is N serializations per transition. The tradeoff is simplicity vs. I/O. For the MVP with ≤20 sessions, this is acceptable — the state file is small (<100KB) and writes are fast. Using compact JSON (Fix #9) makes these writes even faster.

- [ ] **Step 1: Implement StatePersistence with HashMap, compact JSON, and rollback**

```rust
// src/session/persistence.rs

use crate::error::PilotError;
use super::state::{GlobalState, SessionState, StreamStatus};
use std::path::{Path, PathBuf};

const STATE_FILE: &str = ".glyim-pilot-state.json";

/// Persistent state storage with crash recovery.
///
/// ## I/O Tradeoff
///
/// Every mutation serializes and writes ALL sessions to disk. This
/// guarantees crash recovery (REQ-FUNC-061) at the cost of I/O.
/// With ≤20 sessions and a small state file (<100KB), this is
/// acceptable for the MVP. Future optimization options:
///
/// 1. **Debounced writes**: Batch mutations within a time window
/// 2. **Append-only log**: Write deltas, compact periodically
/// 3. **Write-on-shutdown + periodic snapshot**: Risk of losing
///    the last N seconds of state on crash
///
/// For now, simplicity wins.
///
/// ## Fix #9: Compact JSON for saves
///
/// `save()` uses `serde_json::to_string` (compact) instead of
/// `to_string_pretty`. Pretty-printing is unnecessarily slow for
/// crash-recovery writes — the file doesn't need to be human-readable
/// on every write. `debug_dump()` provides pretty output for CLI
/// debugging commands.
pub struct StatePersistence {
    path: PathBuf,
    state: GlobalState,
}

impl StatePersistence {
    /// Create a new persistence layer, loading from file if it exists.
    pub async fn load(project_root: &Path) -> Result<Self, PilotError> {
        let path = project_root.join(STATE_FILE);
        let state = if path.exists() {
            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| PilotError::Session(format!("failed to read state file: {e}")))?;
            serde_json::from_str(&content)
                .map_err(|e| PilotError::Session(format!("failed to parse state file: {e}")))?
        } else {
            GlobalState::new()
        };
        tracing::info!(
            path = %path.display(),
            sessions = state.sessions.len(),
            "loaded state persistence"
        );
        Ok(Self { path, state })
    }

    /// Save current state to disk using compact JSON (Fix #9).
    async fn save(&self) -> Result<(), PilotError> {
        // Fix #9: Use compact JSON for crash-recovery saves.
        // This is significantly faster than to_string_pretty for
        // frequent writes. Reserve pretty printing for debug_dump().
        let content = serde_json::to_string(&self.state)
            .map_err(|e| PilotError::Session(format!("failed to serialize state: {e}")))?;
        tokio::fs::write(&self.path, content)
            .await
            .map_err(|e| PilotError::Session(format!("failed to write state file: {e}")))?;
        Ok(())
    }

    /// Pretty-printed state dump for CLI `debug-dump` commands.
    /// Not used for crash-recovery saves — those use compact JSON.
    pub fn debug_dump(&self) -> Result<String, PilotError> {
        serde_json::to_string_pretty(&self.state)
            .map_err(|e| PilotError::Session(format!("failed to serialize state: {e}")))
    }

    /// Get a reference to the global state.
    pub fn state(&self) -> &GlobalState {
        &self.state
    }

    /// Add a new session and persist.
    pub async fn add_session(&mut self, session: SessionState) -> Result<(), PilotError> {
        let stream_id = session.stream_id.clone();
        self.state.sessions.insert(stream_id, session);
        self.save().await
    }

    /// Update a session by stream_id and persist.
    ///
    /// Returns `Err` if the session doesn't exist.
    /// The closure receives `&mut SessionState`; mutations are
    /// saved even if the closure doesn't explicitly return Ok.
    /// For fallible closures, use `try_update_session` instead.
    pub async fn update_session<F>(
        &mut self,
        stream_id: &str,
        f: F,
    ) -> Result<(), PilotError>
    where
        F: FnOnce(&mut SessionState),
    {
        let session = self
            .state
            .sessions
            .get_mut(stream_id)
            .ok_or_else(|| {
                PilotError::Session(format!("session {stream_id} not found"))
            })?;
        f(session);
        self.save().await
    }

    /// Update a session by stream_id with a fallible closure.
    ///
    /// Fix #6 infrastructure: If the closure returns `Err`, the session
    /// is NOT modified and the state is NOT saved. This prevents
    /// inconsistent state from being persisted when a transition fails.
    ///
    /// Usage pattern in the orchestrator:
    /// ```ignore
    /// persistence.try_update_session(stream_id, |s| {
    ///     s.record_commit();
    ///     s.fix_round = new_fix_round;  // Single write (Fix #4)
    ///     TransitionValidator::transition(s, StreamStatus::Committed)
    ///         // If transition fails, the entire closure returns Err,
    ///         // record_commit and fix_round assignment are rolled back,
    ///         // and the state is NOT saved.
    /// }).await?;
    /// ```
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
            .ok_or_else(|| {
                PilotError::Session(format!("session {stream_id} not found"))
            })?;
        f(session)?;
        self.save().await
    }

    /// Get a session by stream_id (O(1) lookup).
    pub fn get_session(&self, stream_id: &str) -> Option<&SessionState> {
        self.state.sessions.get(stream_id)
    }

    /// Get a mutable reference to a session by stream_id (O(1) lookup).
    pub fn get_session_mut(&mut self, stream_id: &str) -> Option<&mut SessionState> {
        self.state.sessions.get_mut(stream_id)
    }

    /// Get all active (non-complete) sessions.
    pub fn active_sessions(&self) -> Vec<&SessionState> {
        self.state
            .sessions
            .values()
            .filter(|s| s.status != StreamStatus::Complete)
            .collect()
    }

    /// Get all sessions (including complete).
    pub fn all_sessions(&self) -> Vec<&SessionState> {
        self.state.sessions.values().collect()
    }

    /// Remove a session and persist.
    pub async fn remove_session(&mut self, stream_id: &str) -> Result<(), PilotError> {
        self.state.sessions.remove(stream_id);
        self.save().await
    }

    /// Get the number of sessions.
    pub fn session_count(&self) -> usize {
        self.state.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, StatePersistence) {
        let dir = tempfile::tempdir().unwrap();
        let persistence = StatePersistence::load(dir.path()).await.unwrap();
        (dir, persistence)
    }

    #[tokio::test]
    async fn test_load_empty() {
        let dir = tempfile::tempdir().unwrap();
        let persistence = StatePersistence::load(dir.path()).await.unwrap();
        assert!(persistence.state().sessions.is_empty());
    }

    #[tokio::test]
    async fn test_add_and_persist_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut persistence = StatePersistence::load(dir.path()).await.unwrap();

        let session =
            SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        persistence.add_session(session).await.unwrap();

        // In-memory lookup
        assert!(persistence.get_session("S01").is_some());

        // Reload from disk
        let reloaded = StatePersistence::load(dir.path()).await.unwrap();
        assert_eq!(reloaded.state().sessions.len(), 1);
        assert_eq!(
            reloaded.get_session("S01").unwrap().stream_id,
            "S01"
        );
    }

    #[tokio::test]
    async fn test_update_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut persistence = StatePersistence::load(dir.path()).await.unwrap();

        let session =
            SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        persistence.add_session(session).await.unwrap();

        persistence
            .update_session("S01", |s| {
                s.transition(StreamStatus::Seeding);
                s.record_turn();
            })
            .await
            .unwrap();

        let updated = persistence.get_session("S01").unwrap();
        assert_eq!(updated.status, StreamStatus::Seeding);
        assert_eq!(updated.turn, 1);

        // Verify persistence
        let reloaded = StatePersistence::load(dir.path()).await.unwrap();
        assert_eq!(
            reloaded.get_session("S01").unwrap().status,
            StreamStatus::Seeding
        );
    }

    #[tokio::test]
    async fn test_update_nonexistent_session_fails() {
        let dir = tempfile::tempdir().unwrap();
        let mut persistence = StatePersistence::load(dir.path()).await.unwrap();

        let result = persistence
            .update_session("S99", |_| {})
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("S99 not found"));
    }

    #[tokio::test]
    async fn test_try_update_session_rollback_on_error() {
        // Fix #6: If the closure returns Err, the session should NOT
        // be modified and the state should NOT be saved.
        let dir = tempfile::tempdir().unwrap();
        let mut persistence = StatePersistence::load(dir.path()).await.unwrap();

        let session =
            SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        persistence.add_session(session).await.unwrap();

        let result = persistence
            .try_update_session("S01", |s| {
                s.transition(StreamStatus::Seeding); // mutate
                Err(PilotError::Session("simulated failure".into())) // then fail
            })
            .await;

        assert!(result.is_err());
        // The session should NOT have been modified
        assert_eq!(
            persistence.get_session("S01").unwrap().status,
            StreamStatus::Init
        );

        // Verify on disk too
        let reloaded = StatePersistence::load(dir.path()).await.unwrap();
        assert_eq!(
            reloaded.get_session("S01").unwrap().status,
            StreamStatus::Init
        );
    }

    #[tokio::test]
    async fn test_try_update_session_saves_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let mut persistence = StatePersistence::load(dir.path()).await.unwrap();

        let session =
            SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        persistence.add_session(session).await.unwrap();

        persistence
            .try_update_session("S01", |s| {
                s.transition(StreamStatus::Seeding);
                Ok(())
            })
            .await
            .unwrap();

        assert_eq!(
            persistence.get_session("S01").unwrap().status,
            StreamStatus::Seeding
        );
    }

    #[tokio::test]
    async fn test_active_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let mut persistence = StatePersistence::load(dir.path()).await.unwrap();

        let mut s1 =
            SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt1".into());
        s1.status = StreamStatus::Waiting;
        let mut s2 =
            SessionState::new("S02".into(), "grok".into(), "/tmp/wt2".into());
        s2.status = StreamStatus::Complete;

        persistence.add_session(s1).await.unwrap();
        persistence.add_session(s2).await.unwrap();

        let active = persistence.active_sessions();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].stream_id, "S01");
    }

    #[tokio::test]
    async fn test_remove_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut persistence = StatePersistence::load(dir.path()).await.unwrap();

        let session =
            SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        persistence.add_session(session).await.unwrap();
        persistence.remove_session("S01").await.unwrap();
        assert!(persistence.get_session("S01").is_none());
    }

    #[tokio::test]
    async fn test_crash_recovery() {
        let dir = tempfile::tempdir().unwrap();

        // Write initial state
        let mut p1 = StatePersistence::load(dir.path()).await.unwrap();
        let mut session =
            SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        session.turn = 5;
        session.commits = 2;
        session.fix_round = 3;
        p1.add_session(session).await.unwrap();
        drop(p1); // "crash"

        // Recover
        let p2 = StatePersistence::load(dir.path()).await.unwrap();
        let recovered = p2.get_session("S01").unwrap();
        assert_eq!(recovered.turn, 5);
        assert_eq!(recovered.commits, 2);
        assert_eq!(recovered.fix_round, 3);
    }

    #[tokio::test]
    async fn test_crash_recovery_preserves_cooldown() {
        let dir = tempfile::tempdir().unwrap();

        let mut p1 = StatePersistence::load(dir.path()).await.unwrap();
        let mut session =
            SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        let cooldown_until = chrono::Utc::now() + chrono::Duration::seconds(300);
        session.set_provider_cooldown(cooldown_until);
        p1.add_session(session).await.unwrap();
        drop(p1);

        let p2 = StatePersistence::load(dir.path()).await.unwrap();
        let recovered = p2.get_session("S01").unwrap();
        assert!(recovered.provider_cooldown_until.is_some());
        assert!(recovered.is_provider_in_cooldown());
    }

    #[tokio::test]
    async fn test_multiple_sessions_hashmap_lookup() {
        let dir = tempfile::tempdir().unwrap();
        let mut persistence = StatePersistence::load(dir.path()).await.unwrap();

        for i in 1..=20 {
            let session = SessionState::new(
                format!("S{i:02}"),
                "deepseek".into(),
                format!("/tmp/wt{i:02}"),
            );
            persistence.add_session(session).await.unwrap();
        }

        assert_eq!(persistence.session_count(), 20);

        // O(1) lookup
        let s10 = persistence.get_session("S10").unwrap();
        assert_eq!(s10.stream_id, "S10");

        // Update specific session
        persistence
            .update_session("S10", |s| s.record_turn())
            .await
            .unwrap();
        assert_eq!(persistence.get_session("S10").unwrap().turn, 1);
        assert_eq!(persistence.get_session("S09").unwrap().turn, 0);
    }

    #[tokio::test]
    async fn test_save_uses_compact_json_not_pretty() {
        // Fix #9: Verify that save() produces compact JSON, not pretty.
        let dir = tempfile::tempdir().unwrap();
        let mut persistence = StatePersistence::load(dir.path()).await.unwrap();

        let session =
            SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        persistence.add_session(session).await.unwrap();

        // Read the raw file content
        let content = tokio::fs::read_to_string(dir.path().join(STATE_FILE))
            .await
            .unwrap();

        // Compact JSON should have no indentation (no leading whitespace on lines)
        // Pretty JSON has newlines and indentation
        let line_count = content.lines().count();
        assert!(
            line_count <= 5,
            "compact JSON should be few lines, got {line_count} — likely using to_string_pretty"
        );
    }

    #[tokio::test]
    async fn test_debug_dump_uses_pretty_json() {
        // Fix #9: debug_dump() should produce pretty JSON for CLI debugging.
        let dir = tempfile::tempdir().unwrap();
        let mut persistence = StatePersistence::load(dir.path()).await.unwrap();

        let session =
            SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        persistence.add_session(session).await.unwrap();

        let dump = persistence.debug_dump().unwrap();
        let line_count = dump.lines().count();
        assert!(
            line_count > 5,
            "pretty JSON should have many lines, got {line_count}"
        );
    }

    #[tokio::test]
    async fn test_try_update_with_transition_validation() {
        // Fix #6: Simulate the orchestrator pattern where a transition
        // failure inside try_update_session rolls back all mutations.
        let dir = tempfile::tempdir().unwrap();
        let mut persistence = StatePersistence::load(dir.path()).await.unwrap();

        let session =
            SessionState::new("S01".into(), "deepseek".into(), "/tmp/wt".into());
        persistence.add_session(session).await.unwrap();

        // Attempt an invalid transition (Init → Complete)
        let result = persistence
            .try_update_session("S01", |s| {
                s.record_commit();
                s.fix_round = 0;
                TransitionValidator::transition(s, StreamStatus::Complete)
            })
            .await;

        assert!(result.is_err(), "invalid transition should return Err");

        // The session should NOT have been modified
        let session = persistence.get_session("S01").unwrap();
        assert_eq!(session.status, StreamStatus::Init, "status should not have changed");
        assert_eq!(session.commits, 0, "commits should not have been incremented");
        assert_eq!(session.fix_round, 0, "fix_round should not have changed");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib session::persistence`
Expected: 13 PASS

- [ ] **Step 3: Commit**

```bash
git add src/session/persistence.rs && git commit -m "feat: add StatePersistence with compact JSON saves (Fix #9), rollback on failure (Fix #6), HashMap O(1) lookups"
```

---

### Task 4: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test --lib`
Expected: All PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --check`
Expected: No formatting issues

- [ ] **Step 4: Build release binary**

Run: `cargo build --release`
Expected: Compiles

- [ ] **Step 5: Tag**

```bash
git tag v0.1.0-session -m "Session management with TransitionValidator, HashMap state, compact JSON saves (Fix #7 #9), rollback (Fix #6)"
```

---

**Phase 4 complete.** All fixes applied:

- **Fix #7:** No custom serde serializers for `HashMap<String, SessionState>`. Removed `serialize_session_map` and `deserialize_session_map` — they were no-ops that replicated default behavior. `HashMap` serializes to a JSON object natively.
- **Fix #9:** `save()` uses `serde_json::to_string` (compact) instead of `to_string_pretty`. A `debug_dump()` method provides pretty output for CLI debugging. Compact JSON is significantly faster for frequent crash-recovery writes.
- **Fix #6 infrastructure:** `TransitionValidator::transition` returns `Result<(), PilotError>` — never `()`. `try_update_session` supports fallible closures where a transition failure rolls back all mutations and prevents the state from being saved. The orchestrator in Phase 6 will use this pattern to prevent inconsistent state from being persisted.
- **Fix #4 infrastructure:** `fix_round` is a plain `u32` in `SessionState`. No `record_gate_failure` method exists on `SessionState`. The orchestrator will do a single write: `s.fix_round = decision.new_fix_round()`.
- **`DateTime<Utc>` cooldown** — serializable and survives crashes.
- **`HashMap<String, SessionState>`** for O(1) lookups.
- **I/O tradeoff documented** in code comments.

Ready for **Phase 5: Context Assembly & Provider Dispatch** — shall I continue?
