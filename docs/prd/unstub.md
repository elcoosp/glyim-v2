# PRD: Unstubbing Core Compiler Features

## Problem Statement

The Glyim compiler contains multiple “stubbed” features—placeholders that log warnings and skip implementation. These stubs block correct code generation for common language patterns (slice operations, enum matching, for‑loops, constant patterns, built‑in macros, debug info, and proper destructors). End‑users expect a production‑ready compiler where these features work correctly, without silent fallbacks or incomplete lowering.

## Solution

Implement each stubbed area with a production‑quality solution, guided by the architecture of mature compilers (notably Rustc). Replace all `tracing::warn!("STUB: …")` with compile‑time errors (`stub!` macro) and then implement the missing logic. The work is broken into independent but coordinated modules, each testable in isolation.

## User Stories

1. As a compiler user, I want slice projections (e.g., `&array[1..3]`) to generate correct LLVM code, so that subslicing works reliably.
2. As a compiler user, I want enum downcast projections to produce no‑op address calculations, so that accessing enum variants is safe and efficient.
3. As a compiler user, I want `for` loops to desugar to the `Iterator::next` method, so that any type implementing `Iterator` can be used in a loop.
4. As a language implementer, I want a proper trait solver that can answer “does `I` implement `Iterator` and what is its `Item` type?” without shortcuts.
5. As a compiler user, I want constant block patterns (`const { 1+2 }`) in matches to be evaluated to literals, so that const folding works in pattern contexts.
6. As a compiler user, I want `concat!` and `stringify!` macros to follow the correct token‑based semantics, so that metaprogramming is predictable.
7. As a debugger user, I want accurate line number information for macro‑expanded code, so that stepping through generated code is usable.
8. As a systems programmer, I want drop glue for arrays and slices to drop each element (in reverse order), so that resources are not leaked.
9. As a compiler maintainer, I want dead duplicate code (`src/polymorphize/`, `glyim-pipeline/src/mono_cache_new.rs`) removed to reduce maintenance burden.
10. As a user of pattern matching, I want slice patterns (`[a, b, .., c]`) to lower to length checks and element bindings, so that I can destructure slices ergonomically.

## Implementation Decisions

### 1. Slice Projection Codegen (`glyim-codegen`)
- **Module to modify**: `glyim-codegen/src/lib.rs` (`emit_place_address`, `emit_operand`).
- **Decision**: A slice projection `{ start, end }` is lowered to a fat pointer: `{ data: *T, len: usize }`.
  - Compute base address of the array/slice.
  - Compute start index (from `start` place) and multiply by element size.
  - Add offset to base → `data`.
  - Compute length = `end – start` (if `end` missing, use `array_len – start`).
  - Pack `{ data, len }` into an anonymous struct / tuple.
- **Testing**: Unit tests in `glyim-codegen` that generate IR for slice literals and compare against expected pattern (snapshot tests).

### 2. Enum Downcast Projection Codegen
- **Decision**: `ProjectionElem::Downcast` emits no code. It only changes the type view for subsequent field projections. No pointer adjustment is needed because the discriminant and variant data share the same allocation.
- **Implementation**: In `emit_place_address` and `emit_operand`, match on `ProjectionElem::Downcast` and continue without emitting any instructions.
- **Testing**: Compile a match on an enum and verify that field accesses produce the correct offsets (via LLVM IR inspection).

### 3. For‑Loop Desugaring with Iterator Trait
- **New module**: `glyim-solve` (extended) + `glyim-lower` (modified).
- **Decision**: Implement a full SLG‑based trait solver (no shortcuts). The solver must:
  - Recognise lang items (e.g., `Iterator`, `IntoIterator`, `Option`).
  - Support the query `Implemented(I: Iterator)` and extract the associated type `Item`.
- **`iterator_next_fn` implementation**:
  - Use the solver to prove `I: Iterator`.
  - Obtain the `Item` type and the `DefId` of the `next` method.
  - Return `IteratorNextInfo` containing the function ID, substitution, and types.
- **Desugaring**: Follow Rust’s exact desugaring:
  ```rust
  let iter = IntoIterator::into_iter(iterable);
  loop {
      let val = match Iterator::next(&mut iter) {
          Some(val) => val,
          None => break,
      };
      <body>
  }
  ```
- **Testing**: End‑to‑end tests (run‑pass) that iterate over `Vec`, arrays, and custom iterators; verify correctness.

### 4. Constant Block Patterns (`const { … }` in patterns)
- **New module**: `glyim_const_eval` (HIR‑based constant evaluator).
- **Decision**: Evaluate const blocks during HIR → THIR lowering, replacing `PatternKind::ConstBlock` with `PatternKind::Literal`.
- **Supported operations**: Literals, arithmetic, `if`, `match`, calls to `const fn` (once `const fn` is supported).
- **Interface**:
  ```rust
  pub struct ConstEvaluator<'tcx> { ... }
  impl<'tcx> ConstEvaluator<'tcx> {
      pub fn evaluate_expr(&mut self, expr: &glyim_hir::Expr) -> glyim_type::Const;
  }
  ```
- **Integration point**: `lower_pat` calls the evaluator when it sees a `ConstBlock`.
- **Testing**: Unit tests for the evaluator on a suite of constant expressions; pattern‑matching tests that check that `const { 1+2 }` is equivalent to `3`.

### 5. Built‑in Macros: `concat!` and `stringify!`
- **Module**: `glyim-meta/src/expander/mod.rs` (expand_builtin).
- **Decision**:
  - `concat!`: Flatten the token tree, convert each token to its canonical string representation, concatenate, wrap in quotes → `StringLit`.
  - `stringify!`: Take the argument token tree (unexpanded), convert tokens to strings, join with single spaces, wrap in quotes → `StringLit`.
- **Important**: `stringify!` sees the tokens as they appear after parsing but **before** further macro expansion. This matches Rust.
- **Testing**: Macro tests that compare `concat!("a", 1, "+")` and `stringify!(foo(bar))` against expected outputs.

### 6. LLVM Debug Info – Source Locations for Macro‑Expanded Code
- **Module**: `glyim-codegen-llvm/src/debug.rs`.
- **Decision**:
  - Integrate `HygieneCtx` into `LlvmBackend` (passed through the pipeline).
  - Implement `resolve_span_to_location(span: Span) -> Option<(FileId, usize, usize)>` that walks back through macro expansions using `hygiene.remove_mark` until it reaches a root‑context span.
  - Map the root byte range to line/column using the source map.
  - Cache results for performance.
- **Testing**: Compile a program that uses a macro and inspect the generated LLVM `!DILocation` metadata (via `llvm-diff` or snapshot).

### 7. Drop Glue for Arrays and Slices – Full Drop Elaboration
- **Module**: `glyim-pipeline/src/mono_cache.rs` (drop glue generation) + new pass in `glyim-opt` (`ElaborateDrops`).
- **Decision**: Implement full drop elaboration, including drop flags and dataflow analysis.
- **Components**:
  - **Move analysis** (already exists in `glyim-borrowck/move_analysis.rs`) – provides `MovePath`‑level initialization tracking.
  - **Dataflow analyses**: `MaybeInitializedPlaces` and `MaybeUninitializedPlaces`.
  - **Drop flag insertion**: For each place that may be conditionally initialized, create a boolean local.
  - **Elaboration transformation**: Replace `Drop` terminators with conditional branches on drop flags; handle `DropAndReplace` for assignments.
- **Arrays/Slices**: For small constant‑length arrays (≤32), unroll the drop sequence. For larger arrays and all slices, generate a helper `drop_slice` that loops from `len-1` down to 0 and drops each element.
- **Testing**: Unit tests for drop elaboration (MIR before/after snapshots). Run‑pass tests that check destructors run the correct number of times (e.g., using a `Drop` counter).

### 8. Dead Code Removal
- **Files to delete**: 
  - `src/polymorphize/` (already removed by user).
  - `glyim-pipeline/src/mono_cache_new.rs`.
- **Verification**: `grep -r` for references; full test suite pass.
- **Commit**: “chore: remove dead polymorphize and mono_cache_new modules”.

### 9. Slice Pattern Lowering
- **Module**: `glyim-lower/src/lower_rvalue.rs` (match lowering logic).
- **Decision**: Restructure pattern lowering so that slice patterns are handled during match arm lowering (not inside `bind_pattern`). For each slice pattern:
  - Emit a check block that computes the slice length and verifies `len >= prefix_len + suffix_len`.
  - If insufficient, branch to the next arm.
  - Otherwise, compute addresses for each prefix element, load values into temporaries, bind them to pattern variables.
  - For the rest pattern `..`, create a new slice temporary for the subslice.
  - For suffix elements, compute indices from the end and bind similarly.
- **Testing**: Match tests with various slice patterns, checking that bindings receive correct values and that failing patterns skip the arm.

## Testing Decisions

- **Good test = external behavior only** (e.g., run a program and check its output or side effects). Avoid testing internal MIR structure except for regression tests that snapshot IR after a transformation.
- **Modules that will be tested**:
  - `glyim-codegen` – via LLVM IR snapshots and execution tests.
  - `glyim-lower` – via MIR snapshots for for‑loop desugaring, pattern lowering.
  - `glyim-opt` – via MIR before/after snapshots for drop elaboration.
  - `glyim-typeck` – via THIR snapshots for const evaluation.
  - `glyim-meta` – via macro expansion snapshots.
  - `glyim-llvm-codegen` – via debug info metadata checks.
- **Prior art**:
  - Existing snapshot tests in `glyim-test/src/snapshot/snapshots/` for CST and MIR.
  - Run‑pass tests in `tests/run‑pass/` (using the test harness).
  - Unit tests in each crate (e.g., `glyim-typeck/src/unify.rs`).

## Out of Scope

- Full `const fn` implementation (we only need a minimal constant evaluator for patterns; full `const fn` is separate).
- Async drop glue (future work).
- Optimising drop flag elimination (can be added later as a separate pass).
- Supporting slice patterns with bindings inside the rest pattern (e.g., `[a, rest @ .., b]`) – current plan only supports a simple `..` rest without binding; binding can be added later.
- Debug info for variables (only line/column locations for statements are addressed; variable locations are out of scope for now).

## Further Notes

- The trait solver required for for‑loop desugaring is a significant undertaking. We should build it incrementally: first, a minimal solver that handles only the `Iterator` and `IntoIterator` lang items, then expand to full SLG.
- The const evaluator should be designed to be reusable for `const fn` later; keep it separate from the pattern‑specific logic.
- The drop elaboration pass should be placed in `glyim-opt` and run after borrow checking and before codegen.
- After all stubs are implemented, replace all remaining `tracing::warn!(“STUB: …”)` with `stub!` macros to prevent future regressions.
## Wave / Stream Decomposition for Parallel Agent Execution

To parallelize the unstubbing work across multiple AI agents with **zero merge conflicts** when merging a wave, each stream must:

- Modify **disjoint sets of files** (or the same file but different, non‑adjacent functions – to be safe, we avoid same files within a wave).
- Have a **clear completion criterion** (e.g., all tests pass, no stub warnings remain in the touched code).
- Be **mergeable in any order** within the wave.

---

### Wave 1 – Base Infrastructure (no shared files)

| Stream | Name | Crates / Files | Key Modifications | Acceptance Criteria |
|--------|------|----------------|-------------------|----------------------|
| **1.1** | Slice & Downcast Codegen | `glyim-codegen/src/lib.rs` | `emit_place_address`, `emit_operand` handle `ProjectionElem::Slice` (fat pointer) and `ProjectionElem::Downcast` (no‑op) | LLVM IR for slice projection matches expected pattern; enum field access works |
| **1.2** | Macro Fixes (`concat!`, `stringify!`) | `glyim-meta/src/expander/mod.rs` | `expand_builtin` for `Concat` and `Stringify` uses token‑based conversion, not raw source | Snapshot tests for macro expansions pass |
| **1.3** | Const Evaluator & Pattern Folding | New crate `glyim_const_eval`; `glyim-typeck/src/check_pat.rs`; `glyim-typeck/src/check_expr.rs` (if needed) | `ConstEvaluator::evaluate_expr`; `lower_pat` calls it for `PatternKind::ConstBlock`, replaces with `Literal` | Unit tests for const evaluator; pattern‑match tests with `const { … }` |
| **1.4** | Dead Code Removal | `glyim-pipeline/src/mono_cache_new.rs` | Delete the file; verify no references | `grep` finds nothing; full test suite passes |

**No file conflicts** – each stream touches completely different crates.

---

### Wave 2 – Trait Solver & Debug Info (still independent)

| Stream | Name | Crates / Files | Key Modifications | Acceptance Criteria |
|--------|------|----------------|-------------------|----------------------|
| **2.1** | Trait Solver & For‑Loop Desugaring | `glyim-solve/src/*` (new solver), `glyim-lower/src/lower_rvalue.rs` (only `ExprKind::For` arm), `glyim-lower/src/lower.rs` (`LowerCtx::iterator_next_fn`) | Implement SLG‑based solver with lang items; add query `impl Iterator for I`; desugar `for` loop to `IntoIterator::into_iter` + `Iterator::next` | Run‑pass tests with custom iterators; MIR snapshot of desugared loop |
| **2.2** | LLVM Debug Info (Macro Locations) | `glyim-codegen-llvm/src/debug.rs`, `glyim-codegen-llvm/src/lib.rs`, `glyim-pipeline/src/pipeline_context.rs`, `glyim-span/src/hygiene.rs` (no change) | Pass `HygieneCtx` to LLVM backend; `resolve_span_to_location` using `remove_mark`; emit correct `DILocation` | LLVM IR contains accurate line numbers for macro‑expanded code |

**Why no conflict?**  
Stream 2.1 changes `glyim-solve` and only one function in `glyim-lower/src/lower_rvalue.rs` (the `ExprKind::For` match arm).  
Stream 2.2 changes `glyim-codegen-llvm` and pipeline context – disjoint from `lower_rvalue.rs`.  
Even though both modify `glyim-lower` (2.1 touches `lower_rvalue.rs`, 2.2 touches nothing there), they are safe.

---

### Wave 3 – Pattern & Drop Elaboration (still independent)

| Stream | Name | Crates / Files | Key Modifications | Acceptance Criteria |
|--------|------|----------------|-------------------|----------------------|
| **3.1** | Slice Pattern Lowering | `glyim-lower/src/lower_rvalue.rs` (functions `lower_match` and `bind_pattern` – different from `ExprKind::For` arm) | Generate length checks and element loads for `PatternKind::Slice`; bind prefix, suffix, and optional rest | Match tests with slice patterns; compiled program produces correct bindings |
| **3.2** | Full Drop Elaboration | `glyim-opt/src/elaborate_drops.rs` (new pass), `glyim-opt/src/lib.rs`, `glyim-pipeline/src/mono_cache.rs` | Dataflow analyses `MaybeInitializedPlaces`, `MaybeUninitializedPlaces`; insert drop flags; replace `Drop` terminators; generate loop‑based drop for arrays/slices | MIR snapshots show conditional drops; run‑pass tests with `Drop` counters verify order and count |

**Why no conflict?**  
Stream 3.1 modifies `lower_rvalue.rs` but **different functions** than Stream 2.1 (which modified the `ExprKind::For` arm). They are in the same file but distinct match arms and helper functions – git can merge automatically if changes are far apart. To be absolutely conflict‑free, you can merge Wave 2 first, then Wave 3; they are sequential. Within Wave 3, Stream 3.1 and 3.2 modify different crates (`glyim-lower` vs `glyim-opt`), so no conflict.

---

## Agent Execution Instructions

For each stream, provide the agent with:

1. **Scope** – list of files to change (exact paths).
2. **Detailed spec** – what to implement (from the PRD decisions).
3. **Dependencies** – which other streams must be merged first (none within a wave, but waves are sequential).
4. **Test criteria** – specific tests to run (snapshot, run‑pass, unit).
5. **Merge protocol** – create a branch from main, commit with message `feat(stream-<name>): <summary>`, open PR. No conflicts as long as streams in the same wave target disjoint files.

To generate the actual briefs for your agent‑kit, you can run the `generate-stream.sh` script for each stream. The `streams.json` should contain entries like:

```json
[
  {
    "id": "S01",
    "name": "Slice & Downcast Codegen",
    "crate": "glyim-codegen",
    "owned_crates_and_modules": ["glyim-codegen/src/lib.rs"],
    "locked_interfaces": ["Place::ty", "ProjectionElem"],
    "tests": {
      "snapshot": "tests/codegen/slice_projection.g",
      "unit": "glyim-codegen/tests/slice_codegen.rs"
    },
    "upstream": [],
    "downstream": [],
    "scope_summary": "Implement slice projection (fat pointer) and enum downcast (no‑op) in codegen.",
    "mocking": "Use MockCodegen from glyim-test for IR verification."
  },
  ...
]
```

After defining all streams, run `./generate-all-streams.sh` to produce the markdown briefs for each stream, then dispatch each to an agent using the `dispatch.sh` script.

---

## Summary of Waves

| Wave | Streams | Parallel Agents | Merge Order |
|------|---------|----------------|--------------|
| 1 | 1.1, 1.2, 1.3, 1.4 | 4 agents | any order |
| 2 | 2.1, 2.2 | 2 agents | any order (after wave 1 merged) |
| 3 | 3.1, 3.2 | 2 agents | any order (after wave 2 merged) |

This gives **8 parallel agents** across three sequential waves, **zero merge conflicts** because files touched in each wave are disjoint. The only sequential dependency is that Wave 3’s slice pattern lowering uses the same file as Wave 2’s for‑loop desugaring, but they are different functions and git can auto‑merge; still, to be safe, merge Wave 2 completely before starting Wave 3.

Would you like me to generate the full `streams.json` file for all 8 streams?
