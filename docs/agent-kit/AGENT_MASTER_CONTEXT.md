# Glyim Compiler — Master Agent Context

## Project Overview
Glyim is a from‑scratch compiler for a Rust‑like language, written in Rust. The codebase uses **26 crates** organised in layers (edition 2024, resolver = "3").  
You are implementing one stream of work within this project.

## Architecture Rules (NON‑NEGOTIABLE)

1. **No `pub` signature changes.** If a public type or function exists in a crate you don’t own, you MAY NOT modify it. If you need a change, add a `pub(crate)` helper instead.
2. **No new `pub` items in existing modules** without explicit approval. You MAY add new private modules and `pub(crate)` items freely.
3. **No `unsafe` in compiler crates.** Only `glyim-runtime` may contain `unsafe`, and each block must have a `// SAFETY:` proof.
4. **No `todo!()` in non‑test code.** Use `tracing::warn!("STUB: {reason}")` for optional paths that need attention but won’t crash.
5. **All stubs must be visible.** Silent no‑ops (empty match arms, `let _ = x`) are forbidden in implementation code. Every stub must emit a warning on first execution.
6. **Tracing convention:** `trace` for hot paths, `debug` for inference, `info` for phases. Always `skip(self, ctx)`.
7. **Test‑first:** Write all test cases from your stream’s TDD plan **before** implementing. Tests must compile before implementation begins.

## Crate Dependency Rules

- Frontend crates (`glyim-syntax`, `glyim-frontend`, `glyim-hir`, `glyim-def-map`, `glyim-meta`) **never** depend on `glyim-type`.
- `glyim-lsp` depends **only** on `glyim-db` (no LLVM transitive dependency) – checked via Cargo.toml.
- `glyim-db` does **not** use Salsa (removed for v0.1.0 honesty).
- `TyCtxMut` is `!Send + !Sync`. Post‑typeck, only `TyCtx` (frozen, `Send + Sync`) is used.

## Key Type Contracts

- `Ty` **does not** implement `IdxLike`. Construction via `Ty::from_raw()` is `pub(crate)`. Use sentinels: `Ty::ERROR`, `Ty::NEVER`, `Ty::UNIT`, `Ty::BOOL`.
- `TypeLookup` trait bridges `TyCtx` and `TyCtxMut` for display and flag computation.
- `InferVar` has separate index types: `TyVar`, `IntVar`, `FloatVar`. Cross‑kind construction is impossible by type.
- `compute_flags` is generic over `TypeLookup`:  
  `fn compute_flags(kind: &TyKind, ctx: &dyn TypeLookup, depth: u32) -> TypeFlags`.
- `Substitution` is interned as `(u32 index, u16 len)`. Access via `ctx.substitution_args(sub)`. Construction via `TyCtxMut::intern_substitution()`.
- `Place::ty()` takes `&impl TypeLookup` and `&IndexVec<LocalIdx, LocalDecl>`.
- `InferenceTable::new_ty_var`, `new_int_var`, `new_float_var` all take `&mut TyCtxMut`.
- `SimpleTraitSolver::new` takes `&TraitContext`.
- `Database::new` takes `CrateConfig { name: String, target_triple: String, opt_level: u8 }`.

## Error Handling

- Use `GlyimDiagnostic` for all errors. Constructors: `lex_error`, `parse_error`, `type_error`, `borrow_error`, `internal_error`.
- `DiagSink` collects diagnostics with error limiting (default 50).
- `CompResult<T> = Result<T, Vec<GlyimDiagnostic>>`.

## Git Convention

- Branch: `stream-S{XX}/v0.1.0` (e.g., `stream-S01/v0.1.0`)
- Commit format: `stream-S{XX}: description` (e.g., `stream-S01: feat(lex): add float exponent scanning`)
- **Do NOT commit to `main` directly.** Create a PR.
- The first script in a stream **MUST** create and checkout the branch. Subsequent scripts assume the branch is already checked out.

## PR Description Assembly Process (When Stream is Finished)

When the user declares a stream is “finished” or “ready for PR”, you **MUST**:

1. Get the diff from `main` to understand what changed.
2. Get the commit log from `main` to understand the commit history.
3. Assemble a comprehensive PR description following the template below.

### Commands to Run

```bash
# Navigate to the worktree
cd /path/to/glyim-worktrees/stream-SXX

# Get commit log (commits not in main)
git log main..HEAD --oneline > /tmp/sXX_commits.txt

# Get full commit details
git log main..HEAD >> /tmp/sXX_commits_full.txt

# Get diff from main
git diff main..HEAD > /tmp/sXX_diff.txt

# Get diff stats
git diff main..HEAD --stat > /tmp/sXX_stats.txt
```

### PR Description Template

```markdown
## PR: [Stream Name] - [Brief Description]

This PR implements [core functionality] for the Glyim compiler.

### Related Issues
- Stream XX: [Stream Name]
- Required for downstream streams: [list]

### Changes Overview

#### Core Implementation
- [List major features/changes]

#### Bug Fixes
- [List critical fixes]

### Test Coverage (X modules, Y+ tests)

#### [Category 1] Tests (X modules)
- `module1.rs` - Description
- `module2.rs` - Description

#### [Category 2] Tests (X modules)
- `module3.rs` - Description

### Commits

| Commit | Description |
|--------|-------------|
| [hash](url) | Description |
| [hash](url) | Description |

### Files Changed

**Modified:**
- `path/to/file.rs` - Description

**Added (X files):**
- `path/to/file1.rs`
- `path/to/file2.rs`

### Key Design Decisions

1. **Decision 1** - Rationale
2. **Decision 2** - Rationale

### Test Results

```
running Y+ tests across X modules
test result: ok. 0 failed; 0 ignored
```

### Breaking Changes

None (or describe if any)

### Dependencies

- New dev-dependency: `crate-name` (workspace)

### Next Steps

This PR unblocks:
- **SXX: Downstream** - Description

### Checklist

- [x] Code compiles without warnings
- [x] All Y+ tests pass
- [x] API documentation in comments
- [x] No breaking changes to existing APIs
- [x] Stress tests included
- [x] Property-based tests for invariants
- [x] Regression tests for fixed bugs

---

**Branch:** `stream-SXX/v0.1.0` → `main`

**Ready for review!** 🚀
```

### One‑Liner for Quick Collection

```bash
echo "=== COMMITS ===" && git log main..HEAD --oneline && echo "=== STATS ===" && git diff main..HEAD --stat && echo "=== DIFF (first 200 lines) ===" && git diff main..HEAD | head -200
```

## Output Skill: plan‑to‑cat‑scripts

You **MUST** follow the `plan-to-cat-scripts` skill. Key requirements:

- Every message is **exactly one fenced bash code block** – no other text.
- The first script creates branch `stream-SXX/v0.1.0` from `main`.
- Every action is logged with `echo` – **no hash‑comment lines**.
- Write complete files via heredoc with **unique delimiters**.
- Patch trivial single‑line changes with `sed`; all other patches use Python with temp files.
- **Never truncate files.** Set `INCOMPLETE=true` and continue in the next script.
- Compile check runs at the end; failure blocks commit but never halts the script.
- Commit messages are prefixed with `stream-SXX:`.
- When the user pastes an error log, respond with a single surgical fix script.

## Parallel Worktree Workflow (MANDATORY)

To allow multiple streams to run in parallel without branch conflicts, **each stream MUST operate inside its own git worktree**.

- **Worktree location:** `../glyim-worktrees/stream-SXX/` (relative to the main repository root).
- **Branch naming:** `stream-SXX/v0.1.0`
- **Main repository:** The main clone is never modified directly; all changes happen in the worktree.

**Why worktrees?** Worktrees allow multiple branches to be checked out simultaneously in separate directories, avoiding the need to stash or switch branches.

### First Script Worktree Setup (CORRECTED)

**CRITICAL:** You cannot add a worktree for a branch that is already checked out elsewhere. The correct approach:

```bash
STREAM_ID="S01"
WORKTREE_DIR="../glyim-worktrees/stream-${STREAM_ID}"
BRANCH_NAME="stream-${STREAM_ID}/v0.1.0"

echo "Setting up worktree for stream ${STREAM_ID}"

# Check if worktree already exists
if [ -d "$WORKTREE_DIR" ]; then
  echo "Worktree directory already exists, reusing it"
else
  # Create worktree with a detached HEAD (--detach), then create the branch
  git worktree add --detach "$WORKTREE_DIR" main
  cd "$WORKTREE_DIR" || { echo "ERROR: cannot cd to $WORKTREE_DIR"; exit 1; }
  git checkout -b "$BRANCH_NAME"
  cd - > /dev/null
fi

cd "$WORKTREE_DIR" || { echo "ERROR: cannot cd to $WORKTREE_DIR"; exit 1; }
git checkout "$BRANCH_NAME" 2>/dev/null || git checkout -b "$BRANCH_NAME"

# Pull latest main changes (optional)
git fetch origin main 2>/dev/null || true
git merge main --no-edit 2>/dev/null || true
```

**Alternative (simpler) – use a unique base branch:**

```bash
git branch "worktree-base-${STREAM_ID}" main 2>/dev/null || true
git worktree add "$WORKTREE_DIR" "worktree-base-${STREAM_ID}"
cd "$WORKTREE_DIR" || exit 1
git checkout -b "$BRANCH_NAME"
```

**Important:**
- Never use `git worktree add "$WORKTREE_DIR" main` directly – this fails because `main` is already checked out.
- Always use `--detach` or a unique base branch.
- The worktree directory is **outside** the main repository (sibling directory `glyim-worktrees/`).

## PR Description Collection Script

When a stream is finished, run this to collect all needed data:

```bash
#!/usr/bin/env bash
set -euo pipefail

STREAM_ID="${1:-S03}"
WORKTREE_DIR="/path/to/glyim-worktrees/stream-${STREAM_ID}"

if [ ! -d "$WORKTREE_DIR" ]; then
    echo "ERROR: Worktree not found at $WORKTREE_DIR"
    exit 1
fi

cd "$WORKTREE_DIR"

echo "=== Collecting PR data for stream $STREAM_ID ==="
echo ""
echo "=== COMMIT LOG ==="
git log main..HEAD --oneline
echo ""
echo "=== FULL COMMITS ==="
git log main..HEAD
echo ""
echo "=== DIFF STATS ==="
git diff main..HEAD --stat
echo ""
echo "=== DIFF (first 500 lines) ==="
git diff main..HEAD | head -500

# Copy full diff to clipboard (macOS)
git diff main..HEAD | pbcopy
echo ""
echo "✅ Full diff copied to clipboard"

# Save to files
git log main..HEAD --oneline > "/tmp/s${STREAM_ID}_commits.txt"
git diff main..HEAD --stat > "/tmp/s${STREAM_ID}_stats.txt"
echo "✅ Saved to /tmp/s${STREAM_ID}_commits.txt and /tmp/s${STREAM_ID}_stats.txt"
```

---

**This master context is authoritative.** Always refer to it when uncertain about project rules or conventions.
