# Glyim Pilot — Technology Stack & Architecture Implementation Guide (Updated)

This remastered stack integrates all review feedback: `blake3` replaces the deprecated `md-5`, every dependency is pinned to a current stable version, and three missing crates (`uuid`, `similar`, `dirs`) have been added to fulfil protocol, self-review, and cross‑platform needs. No deprecations, no release candidates—only solid, widely‑used releases.

---

## CLI: Rust Crate Selection

### Core Framework

| Component | Crate | Version | Why |
|-----------|-------|---------|-----|
| CLI Parser | `clap` | 4.5 | De facto standard; derive macros; subcommand support for `dispatch`, `wave`, `status`, `preflight` |
| Async Runtime | `tokio` | 1 | Full async runtime; process spawning; timer; sync primitives |
| Error Handling | `anyhow` + `thiserror` | 1 / 2 | `anyhow` for application errors; `thiserror` for library‑style typed errors in protocol/parser |
| Serde | `serde` + `serde_json` + `toml` | 1 / 1 / 0.8 | Config parsing, protocol messages, state persistence |
| Tracing | `tracing` + `tracing-subscriber` | 0.1 / 0.3 | Structured logging; span‑based per‑session traces; `skip(self, ctx)` per project convention |

### WebSocket Server

| Crate | Version | Why |
|-------|---------|-----|
| `tokio-tungstenite` | 0.29 | Lightweight WebSocket server on tokio; no hyper/axum overhead; we only need one connection (the extension) |
| `futures-util` | 0.3 | Stream/Sink traits for tungstenite; `SplitSink`/`SplitStream` for bidirectional handling |

### Parser: glyim-ops

| Crate | Version | Why |
|-------|---------|-----|
| `winnow` | 1.0 | Fast combinator parser; stable API; better error messages than `nom`; zero‑copy; excellent for line‑oriented protocols |
| `regex` | 1.11 | For provider error pattern matching and selector validation |

**Why `winnow` over `nom`**: Winnow is the successor by the same author (Geal). It reached 1.0, guaranteeing API stability, offers better error recovery, and has a more maintainable design than `nom`.

### Process Execution

| Crate | Version | Why |
|-------|---------|-----|
| `tokio` (built‑in `process`) | 1 | `tokio::process::Command` for async subprocess execution; no extra crate needed |

### Git Operations

Shell out to `git` CLI — **not `git2`**. The `git2` crate is massive, hard to link (libgit2), and we only need 8 git operations. Shelling out is simpler, more portable, and matches the manual workflow exactly:

```bash
git worktree add --detach <dir> main
git checkout -b stream-XX/v0.1.0
git add -A
git commit -m "message"
git push -u origin stream-XX/v0.1.0
gh pr create --base main --head stream-XX/v0.1.0 --title "..." --body "..."
git status --porcelain
git diff main..HEAD
git log main..HEAD --oneline
```

### Context Assembly

| Crate | Version | Why |
|-------|---------|-----|
| `walkdir` | 2 | Directory traversal for discovering source files |
| `glob` | 0.3 | Glob pattern matching for file discovery |
| `ignore` | 0.4 | Respect `.gitignore` when scanning source files |
| `pulldown-cmark` | 0.13 | Parse markdown in briefs and skill documents; extract code blocks |

*Removed from the spec*: `handlebars` (template rendering not required for initial stream briefs) and `syntect` (syntax highlighting detection overkill for code‑block extraction).

### Quality Gates, Diff & Hashing

| Crate | Version | Why |
|-------|---------|-----|
| `blake3` | 1.8 | **Replaces `md‑5`**. Modern, cryptographic‑safe hashing for script dedup tracking. Faster than MD5 on modern hardware, zero collision risk. |
| `similar` | 3 | High‑quality text diffing for the self‑review gate (REQ‑FUNC‑051). Dependency‑free, supports multiple algorithms (Myers, Patience, LCS). |
| `uuid` | 1 | Generates unique, random session/stream IDs (`v4` feature) for the WebSocket protocol and crash recovery. |
| `dirs` | 6 | Locate the project directory, configuration, and state files across Linux, macOS, and Windows. |
| `strip-ansi-escapes` | 0.2 | Clean cargo output for error feedback |
| `chrono` | 0.4 | Timestamps in session state and commit messages |
| `rand` | 0.10 | Jitter in retry backoff; updated RNG traits (`rand::rngs::StdRng` uses ChaCha20) |
| `console` | 0.16 | Terminal styling for status dashboard (colors, progress bars) – note: enable `std` feature if `default-features = false` |
| `indicatif` | 0.18 | Progress bars for commit pipeline and verification |
| `comfy-table` | 7 | Beautiful terminal tables for status display |

### Security & Validation

| Crate | Version | Why |
|-------|---------|-----|
| `path-clean` | 1.0 | Path normalization for worktree containment checks |
| `dunce` | 1.0 | Windows‑compatible canonicalization |

### State Persistence

| Crate | Version | Why |
|-------|---------|-----|
| `serde_json` | 1 | State file serialization |
| `tokio` (built‑in `fs`) | 1 | Async file I/O |
| `notify` | 8 (optional) | Watch `.glyim-pilot-state.json` for external changes (major update; now requires Rust 1.77+) |

### Testing

| Crate | Version | Why |
|-------|---------|-----|
| `proptest` | 1.11 | Property‑based testing for parser (fuzz all directive combinations) |
| `tempfile` | 3 | Temporary directories for applier and executor tests |
| `tokio-test` | 0.4 | Async test utilities |
| `mockall` | 0.13 | Mock trait implementations (ProviderPool, GateRunner) |
| `assert_cmd` | 2 | CLI integration testing |
| `predicates` | 3 | Fluent assertions for assert_cmd |
| `httpmock` | 0.7 | Mock HTTP server for simulating provider responses (E2E) |
| `pretty_assertions` | 1 | Colourful diff output for failed tests |

### Code Quality (CI)

| Tool | Version | Why |
|------|---------|-----|
| `cargo-clippy` | – | Linting |
| `cargo-fmt` | – | Formatting |
| `cargo-audit` | – | Vulnerability scanning |
| `cargo-mutants` | 25.x | Mutation testing of pilot’s own test suite |

---

## Extension: CRXJS + Vite + TypeScript

The extension stack remains stable; the only change is that the `package.json` devDependencies are pinned to current versions:

```json
{
  "devDependencies": {
    "@crxjs/vite-plugin": "^2.0.0-beta.28",
    "@types/chrome": "^0.0.287",
    "typescript": "^5.7.0",
    "vite": "^6.1.0"
  }
}
```

All provider adapters, the WebSocket client, and the protocol types continue to be implemented in TypeScript as described in the original architecture.

---

## Complete `Cargo.toml`

```toml
[package]
name = "glyim-pilot"
version = "0.1.0"
edition = "2024"
resolver = "3"
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
chrono = "0.4"

# Hashing – BLAKE3 replaces MD5
blake3 = "1.8"

# Random (jitter)
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

# Diff generation for self-review gate
similar = "3"

# Cross‑platform project directories
dirs = "6"

# State persistence (optional file watcher)
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

---

## Summary of Changes from Original

| Change | Rationale |
|--------|-----------|
| `md-5` → `blake3` | MD5 is cryptographically broken; BLAKE3 is faster, collision‑safe, and actively maintained |
| `tokio-tungstenite` 0.24 → 0.29 | 5 versions of bug fixes and protocol improvements |
| `winnow` 0.7 → 1.0 | Stable API; better error messages |
| `console` 0.15 → 0.16 | Minor updates; `std` feature must be enabled manually if needed |
| `indicatif` 0.17 → 0.18 | Minor updates |
| `pulldown-cmark` 0.12 → 0.13 | CommonMark spec updates and performance improvements |
| `rand` 0.8 → 0.10 | RNG trait updates; `StdRng` now uses ChaCha20 |
| `notify` 7 → 8 (optional) | Major update; MSRV 1.77 |
| **Added** `uuid` | Proper unique session ID generation for WebSocket protocol and recovery |
| **Added** `similar` | Text diff generation for the self‑review gate |
| **Added** `dirs` | Cross‑platform resolution of project/config directories |
| Removed `handlebars`, `syntect` | Not required for initial prompt assembly; removed to keep dependency tree lean |
