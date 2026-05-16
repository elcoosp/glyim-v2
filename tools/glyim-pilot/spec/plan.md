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
