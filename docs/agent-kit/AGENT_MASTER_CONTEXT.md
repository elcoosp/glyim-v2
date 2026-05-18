# Glyim Compiler — Master Agent Context

## Project Overview
Glyim is a from‑scratch compiler for a Rust‑like language, written in Rust. The codebase uses **26 crates** organised in layers (edition 2024, resolver = "3").  
You are implementing one stream of work within this project.

## Architecture Rules (NON‑NEGOTIABLE)

1. **No `pub` signature changes.** If a public type or function exists in a crate you don't own, you MAY NOT modify it. If you need a change, add a `pub(crate)` helper instead.
2. **No new `pub` items in existing modules** without explicit approval. You MAY add new private modules and `pub(crate)` items freely.
3. **No `unsafe` in compiler crates.** Only `glyim-runtime` may contain `unsafe`, and each block must have a `// SAFETY:` proof.
4. **No `todo!()` in non‑test code.** Use `tracing::warn!("STUB: {reason}")` for optional paths that need attention but won't crash.
5. **All stubs must be visible.** Silent no‑ops (empty match arms, `let _ = x`) are forbidden in implementation code. Every stub must emit a warning on first execution.
6. **Tracing convention:** `trace` for hot paths, `debug` for inference, `info` for phases. Always `skip(self, ctx)`.
7. **Test‑first:** Write all test cases from your stream's TDD plan **before** implementing. Tests must compile before implementation begins.
8. **Shared files are append‑only.** Files that multiple streams may modify — especially `src/tests/mod.rs` and `src/lib.rs` — MUST be modified using the safe‑append pattern (Python script that reads existing content and only adds new lines if absent). NEVER use `cat >` (overwrite) on these files. Overwriting `tests/mod.rs` silently deletes other streams' test registrations, which is an unrecoverable data loss.

---

## Code Quality Mandate (NON‑NEGOTIABLE)

Every piece of Rust code produced in any stream is subject to a ruthless review across six dimensions. Before committing, mentally run the following checklist. **If any item fails, fix it — do not ship.**

The target bar is: *a senior principal engineer reads the diff and thinks "this is clean, correct, and maintainable."*

---

### 1. Correctness & Compilation Safety

**Goal:** The code must compile with `cargo check --all-targets` and `cargo clippy -- -D warnings` with zero errors and zero warnings. Logic must be provably correct, not just "probably fine".

**Rules:**

- **Exhaustive `match`.** Every `match` on an enum must cover all variants explicitly. Do not use a catch‑all `_` arm unless the remaining variants are genuinely identical *and* you leave a comment explaining why.
- **No integer arithmetic without overflow consideration.** Use `checked_add`, `saturating_add`, or `wrapping_add` where overflow is conceivable. Bare `+` on `usize`/`u32` is forbidden in index arithmetic.
- **No silent lossy conversions.** `as usize` and `as u32` casts are banned unless the value is provably within range; use `try_from` or a named conversion function with a documented panic/error contract.
- **No `unwrap()` / `expect()` in non‑test, non‑infallible code.** An `expect()` is allowed only when the caller can *statically prove* the `Option`/`Result` is `Some`/`Ok` — add an `// INVARIANT: <reason>` comment on the same line. Use `?` or propagate `GlyimDiagnostic` otherwise.
- **No logic inversion bugs.** Every `if !cond` or `unless` pattern must have a unit test that exercises *both* branches. If you write a predicate, write a test that catches a sign flip.
- **All error paths return `GlyimDiagnostic`.** Never swallow an error into `()` or a default value. If recovery is intentional, document it with `// RECOVERY: <why this is safe>`.
- **Off‑by‑one discipline.** Spans, slices, and index ranges must use half‑open intervals `[lo, hi)` consistently. Any closed range must be called out with a comment.
- **No dead code.** `#[allow(dead_code)]` is forbidden in production paths. If a function is not yet called, it is not yet needed — don't write it.

**Self‑check before committing:**
```
cargo clippy --all-targets -- -D warnings
cargo test --all
```

---

### 2. Boundaries & Contracts

**Goal:** Every public and `pub(crate)` item is a *contract*, not an implementation detail. A caller should never need to read the body to use the function correctly.

**Rules:**

- **Document every `pub` and `pub(crate)` item.** At minimum: what it does, what it expects (preconditions), what it returns (postconditions), and what it emits (side effects: diagnostics, tracing spans, mutations). Use `///` doc comments, not `//`.
- **State preconditions as `debug_assert!`.** If a function requires `idx < self.len()`, add `debug_assert!(idx < self.len(), "…")` as the first line. This makes implicit contracts explicit *and* catches violations in debug builds.
- **No leaking internals.** A `pub(crate)` function must not return a `&mut` to an internal field that callers can corrupt. Return a typed wrapper or a copy.
- **Infallible vs. fallible is a type-level choice.** A function that can fail returns `Result<T, GlyimDiagnostic>` or `CompResult<T>`. A function that panics on contract violation is documented as such. Never mix the two silently.
- **No God functions.** Any function longer than ~60 lines or with cyclomatic complexity > 10 must be decomposed. Extract named helper functions with their own contracts.
- **`Default` must be meaningful.** If you `#[derive(Default)]`, the default value must be a valid, usable instance — not a half‑initialised placeholder. If no valid default exists, do not derive it; require explicit construction.

---

### 3. Modularity & Separation of Concerns

**Goal:** Each module does one thing. Dependencies point inward (toward stable abstractions), never outward (toward volatile implementations).

**Rules:**

- **One concept per module.** A `.rs` file that handles both parsing *and* type inference, or both lowering *and* codegen, is wrong. Split it.
- **Acyclic module graph.** Within a crate, module `A` importing from module `B` which imports from `A` is forbidden. Structure: data types → algorithms → drivers, never the reverse.
- **Traits as seams.** Any place where two subsystems communicate, the dependency must flow through a trait, not a concrete type. This enables independent testing of both sides.
- **No ambient state.** No `static mut`, no `thread_local!` in compiler logic, no hidden global singletons. All state flows through explicit parameters (`&mut TyCtxMut`, `&mut DiagSink`, etc.).
- **Feature cohesion.** If you add a helper function, it belongs in the module whose *data* it primarily operates on, not the module that first needed it.
- **Test isolation.** Every non‑trivial function must be testable without spinning up the entire compiler pipeline. If it isn't, extract a pure sub‑function that is.

---

### 4. Performance & Resource Efficiency

**Goal:** Compiler performance is a feature. Allocate intentionally; never accidentally.

**Rules:**

- **No O(n²) in disguise.** Nested loops over compiler-managed collections (`Vec`, `IndexVec`, `HashMap`) must be justified. If the inner loop is bounded by a small constant (< 8), document it. Otherwise, redesign.
- **Intern aggressively.** Strings, types, and substitutions that escape a single function call must be interned. Never store a `String` where a `Symbol` or `Ty` (interned) will do.
- **Avoid cloning across hot paths.** `Clone` on a non‑`Copy` type in a path called per‑expression or per‑statement is a red flag. Profile first, but default to borrowing.
- **Pre‑allocate collections.** When the approximate size is known at construction time, use `Vec::with_capacity`, `HashMap::with_capacity_and_hasher`, etc. A `push` into a default `Vec::new()` in a tight loop will trigger repeated reallocation.
- **Bound recursion depth.** Every recursive function that follows user‑provided structure (types, expressions, patterns) must track depth and return an error diagnostic at a configurable limit (default 128). This prevents stack overflows on adversarial input.
- **No redundant traversals.** If you need two properties of the same node, compute them in one pass. Two `for` loops over the same `Vec` is almost always reducible to one.

---

### 5. Debuggability & Observability

**Goal:** When something goes wrong in a 50,000‑line compilation, a developer must be able to trace it in under five minutes.

**Rules:**

- **Every phase boundary gets an `info!` span.** Use `tracing::info_span!("phase_name", crate = %name)` at the start of each compiler phase. The span must be entered with `.entered()` so it appears in nested traces.
- **Every non‑trivial decision gets a `debug!` log.** Type inference decisions, trait resolution results, and MIR lowering choices must be logged at `debug` level with enough context to reconstruct the reasoning.
- **Diagnostics carry full context.** A `GlyimDiagnostic` must include: the source span, the human‑readable message, and at least one note explaining *why* the error occurred (not just *what* went wrong). "type mismatch" is not a message; "expected `i32`, found `u8` because the return type of `foo` is declared as `i32`" is.
- **No silent fallbacks.** If a lookup fails and you substitute a sentinel (`Ty::ERROR`), emit a `tracing::debug!` log at that point. Sentinels silently propagating is the number‑one cause of confusing cascading errors.
- **Assertions in debug builds.** Use `debug_assert!` liberally on invariants that are expensive to check in release. They are free in production and priceless during development.
- **`Display` on all domain types.** Every type that appears in a diagnostic message must implement `std::fmt::Display` (via the `TypeLookup` trait where a context is needed). `{:?}` in user-visible output is forbidden.

---

### 6. Elegance & Hack‑Free Design

**Goal:** The implementation is the simplest correct solution. No clever tricks. No "temporary" workarounds. No accidental complexity.

**Rules:**

- **No `#[allow(...)]` without a justification comment.** If you suppress a warning, explain in the same line why it is safe to do so. `#[allow(clippy::too_many_arguments)]` followed by a 12‑argument function is not allowed — refactor into a builder or a context struct.
- **Builder or context struct over long argument lists.** More than 4 parameters to a function is a code smell. Group related parameters into a typed struct. This also makes future additions non‑breaking.
- **No stringly‑typed logic.** Never branch on `&str` or `String` values for compiler-internal decisions. Use enums. If the string comes from user source, intern it to a `Symbol` immediately.
- **Pattern-match, don't interrogate.** Prefer `if let Some(x) = opt { … }` and `match` over `.is_some()` followed by `.unwrap()`. Rust's type system is your friend — use it.
- **No boolean traps.** A function `fn process(node: Node, is_lvalue: bool, is_type_pos: bool)` is a trap. Each `bool` parameter is a hidden enum. Define `enum Position { LValue, RValue }` and `enum Context { TypePosition, ExprPosition }` and use them.
- **Newtype wrappers for distinct indices.** Never use bare `u32` or `usize` as an index into more than one collection. Wrap each in a named newtype (`LocalIdx`, `DefId`, `HirId`). Cross‑kind indexing must be a compile error.
- **Derive traits, don't implement them manually unless necessary.** `PartialEq`, `Eq`, `Hash`, `Clone`, `Copy`, `Debug` — derive them. Manual implementations of structural traits must be accompanied by a comment explaining why the derived version is wrong.
- **No speculative code.** Do not implement functionality that is not required by the current stream's TDD plan. YAGNI is a hard rule here. Speculative code creates maintenance burden and untested surface area.

---

### Applying the Mandate: Workflow

When implementing a feature or fixing a bug, follow this order:

1. **Plan** — write out the types, traits, and function signatures in comments before writing bodies. Run the Boundaries & Contracts checklist mentally.
2. **Test** — write the tests per the TDD plan. They must compile (but can fail) before step 3.
3. **Implement** — write the minimum correct implementation that makes the tests pass. Apply all six dimensions above.
4. **Refactor** — clean up: rename, extract helpers, add doc comments, add `debug_assert!`s, add tracing spans.
4.5 **Integrity check** — Before committing, verify no test modules were lost:
   ```bash
   for crate_dir in crates/*/; do
     mod_file="${crate_dir}src/tests/mod.rs"
     [ -f "$mod_file" ] || continue
     for test_file in "${crate_dir}src/tests/"*.rs; do
       [ -f "$test_file" ] || continue
       module_name=$(basename "$test_file" .rs)
       [ "$module_name" = "mod" ] && continue
       grep -q "mod ${module_name}" "$mod_file" || echo "ORPHANED: $test_file not in mod.rs!"
     done
   done
   ```
5. **Verify** — `cargo clippy -- -D warnings` and `cargo test --all` must both pass clean.
6. **Review yourself** — read the diff as if you are a merciless senior reviewer. Would you approve it? If not, fix it before committing.

**If you are uncertain whether a design decision meets this mandate, the answer is: make it simpler, more explicit, and better typed. Complexity is never the right default.**

---

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

- Branch: `stream-{XX}/v0.1.0` (e.g., `stream-S01/v0.1.0`)
- Commit format: `stream-{XX}: description` (e.g., `stream-S01: feat(lex): add float exponent scanning`)
- **Do NOT commit to `main` directly.** Create a PR.
- The first script in a stream **MUST** create and checkout the branch. Subsequent scripts assume the branch is already checked out.

## PR Description Assembly Process (When Stream is Finished)

When the user declares a stream is "finished" or "ready for PR", you **MUST**:

1. Get the diff from `main` to understand what changed.
2. Get the commit log from `main` to understand the commit history.
3. Assemble a comprehensive PR description following the template below.

### Commands to Run

```bash
# Navigate to the worktree
cd /path/to/glyim-worktrees/stream-XX

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

**Branch:** `stream-XX/v0.1.0` → `main`

**Ready for review!** 🚀
```

### One‑Liner for Quick Collection

```bash
echo "=== COMMITS ===" && git log main..HEAD --oneline && echo "=== STATS ===" && git diff main..HEAD --stat && echo "=== DIFF (first 200 lines) ===" && git diff main..HEAD | head -200
```

## Output Skill: plan‑to‑cat‑scripts

You **MUST** follow the `plan-to-cat-scripts` skill. Key requirements:

- Every message is **exactly one fenced bash code block** – no other text.
- The first script creates branch `stream-XX/v0.1.0` from `main`.
- Every action is logged with `echo` – **no hash‑comment lines**.
- Write complete files via heredoc with **unique delimiters**.
- Patch trivial single-line changes with `sed`; all other patches use Python with temp files.
- **Never truncate files.** Set `INCOMPLETE=true` and continue in the next script.
- Compile check runs at the end; failure blocks commit but never halts the script.
- Commit messages are prefixed with `stream-XX:`.
- When the user pastes an error log, respond with a single surgical fix script.
- **NEVER use `cat >` to overwrite `src/tests/mod.rs`** — always use the safe-append pattern from the skill's section 1.5. This file is shared across streams; overwriting it silently deletes other agents' test registrations.
- **ALWAYS run the full crate test suite** (`cargo test -p <crate>`), not just your own test module.
- **ALWAYS check for orphaned test files** before committing — `.rs` files in `src/tests/` that are not declared in `mod.rs` indicate a previous overwrite.

## Parallel Worktree Workflow (MANDATORY)

To allow multiple streams to run in parallel without branch conflicts, **each stream MUST operate inside its own git worktree**.

- **Worktree location:** `../glyim-worktrees/stream-XX/` (relative to the main repository root).
- **Branch naming:** `stream-XX/v0.1.0`
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
