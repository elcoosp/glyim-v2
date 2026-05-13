# Glyim v0.1.0 — Maximum Parallelism TDD Dispatch Plan

To achieve maximum parallelism, we slice the work by **strict contract boundaries**. Since you have bootstrapped all contracts (public interfaces are locked), AI agents can work in parallel by **mocking upstream dependencies** based on those contracts. They do not need to wait for upstream streams to finish coding; they only need the contracts to write tests.

---

## 1. Pre-Work: Contract Lock (1.5 Hours — Do This Yourself)

Before dispatching, apply the 6 hardening fixes from the review. Then lock the interfaces:

1. Run `cargo check --workspace` — must pass.
2. Tag the commit: `git tag contracts-locked-v0.1.0`.
3. Create a `CONTRACTS.md` file at the root listing every `pub` function/type across all 25 crates. Agents may not alter anything in this file.

---

## 2. The 20 Streams (Maximum Granularity)

Streams are grouped into **Execution Waves** based on true structural dependencies, but agents can start coding and testing immediately in any wave by mocking the inputs using the locked contracts.

### Wave 1: Zero Dependencies (8 Parallel Streams)
*These depend ONLY on the locked Layer 0/1 contracts (`glyim-core`, `glyim-span`, `glyim-diag`, `glyim-syntax`, `glyim-type`).*

| Stream ID | Name | Scope |
|-----------|------|-------|
| S01 | Lexer | Complete lexer, validate all tokens, error recovery |
| S02 | TyCtx-Interning | `TyCtxMut` construction, `mk_*` methods, substitution interning, freeze |
| S03 | MIR-Core | `Body`, `Place::ty()`, `Statement`, `Terminator`, `SwitchTargets` |
| S04 | Layout-Engine | `SimpleLayoutComputer` for all primitives, reject unsized |
| S05 | Unification | `InferenceTable`, unify all `InferVar` kinds, `fully_resolve` |
| S06 | MIR-Interpreter | Execute arithmetic, control flow, calls, allocations |
| S07 | Bytecode-Backend | `BytecodeBackend` emitting opcodes from hand-crafted MIR |
| S08 | LLVM-Backend | `LlvmBackend` generating IR from hand-crafted MIR |

### Wave 2: Mild Dependencies (4 Parallel Streams)
*These depend on Wave 1 outputs, but agents can mock the Wave 1 outputs using the locked contracts.*

| Stream ID | Name | Scope |
|-----------|------|-------|
| S09 | Parser | Uses `Token` stream (mock if S01 incomplete) |
| S10 | TypeDisplay-Flags | `PrintTy`, `compute_flags`, `TypeLookup` trait mocking |
| S11 | TraitSolver | `TraitContext`, `SimpleTraitSolver`, `FulfillmentCtx` |
| S12 | HIR-Lowering | CST → HIR conversion (mock CST if S09 incomplete) |

### Wave 3: Heavy Dependencies (5 Parallel Streams)
*These combine multiple streams. Agents MUST mock upstream structs (e.g., hand-build `CrateHir` for S13, hand-build `thir::Body` for S14).*

| Stream ID | Name | Scope |
|-----------|------|-------|
| S13 | DefMap | Path resolution, scope population |
| S14 | Typeck-Driver | Orchestrate `TypeckCtx`, obligation collection, freeze |
| S15 | MIR-Lowering | THIR → MIR (hand-craft THIR for tests) |
| S16 | Borrowck | Extract constraints, check conflicts (hand-craft MIR for tests) |
| S17 | MIR-Opt | Constant propagation, DCE, CFG simplification |

### Wave 4: Integration (3 Parallel Streams)
*Wire everything together. Run end-to-end.*

| Stream ID | Name | Scope |
|-----------|------|-------|
| S18 | Pipeline | `compile_file` driving all 9 phases |
| S19 | LSP | `did_open`, `did_change`, diagnostics, URI handling |
| S20 | CLI | Argument parsing, backend selection, miette rendering |

---

## 3. Stream TDD Subplans

Each stream includes its **Test Suite** (write first), **Mocking Strategy** (how to test independently), and **Definition of Done**.

### S01: Lexer
**Mocking:** None needed. Source string in, `Vec<Token>` out.
**Tests:**
```
S01-T01: Lex all 30+ keywords → correct SyntaxKind
S01-T02: Lex integer literals → decimal, hex (0x), binary (0b), octal (0o)
S01-T03: Lex integer suffixes → 42u8, 0xFFi32
S01-T04: Lex float literals → 3.14, 1e10, 1.5e-3
S01-T05: Lex float error recovery → 1e, 1e+, 1e- → Error diagnostic + IntLit fallback
S01-T06: Lex string/char escapes → "\n", '\x41'
S01-T07: Lex nested block comments → /* /* */ */
S01-T08: Lex all operators → +=, -=, *=, ::, ->, =>, etc.
S01-T09: Lex unexpected character → Error diagnostic
S01-T10: Roundtrip: source → tokens → source text matches
```
**DoD:** Zero `todo!` in lexer. All tests pass.

### S02: TyCtx-Interning
**Mocking:** None needed. `Interner::new()` is standalone.
**Tests:**
```
S02-T01: Pre-interned sentinels at correct indices → Ty::ERROR.index() == 0
S02-T02: Freeze verifies sentinels → corrupt data → panic
S02-T03: mk_ref creates Ref; roundtrip via ty_kind → TyKind::Ref(_, _, Mut)
S02-T04: mk_adt with Substitution → args roundtrip
S02-T05: mk_fn_ptr → FnSig roundtrip
S02-T06: Substitution interning deduplicates → same args = same index
S02-T07: Substitution with 10 args → no panic
S02-T08: TyCtxMut is !Send + !Sync → compile_fail test
S02-T09: TyCtx is Send + Sync → compiles
```
**DoD:** All `mk_*` methods implemented. `freeze()` works.

### S03: MIR-Core
**Mocking:** Mock `TypeLookup` for `Place::ty()`.
**Tests:**
```
S03-T01: Body::dummy creates valid structure → Return terminator
S03-T02: Place::ty() for Deref on Ref → inner Ty
S03-T03: Place::ty() for Deref on RawPtr → inner Ty
S03-T04: Place::ty() for Field on Tuple → correct arg Ty
S03-T05: Place::ty() for Index on Array → inner Ty
S03-T06: Place::ty() for Index on Slice → inner Ty
S03-T07: Place::ty() error fallback → Deref on int → Ty::ERROR
S03-T08: SwitchTargets::if_switch → correct branches
```
**DoD:** `Place::ty()` handles all `ProjectionElem` variants without cloning `TyKind`.

### S04: Layout-Engine
**Mocking:** Mock `TyCtx` (or use `test_frozen_ty_ctx()`).
**Tests:**
```
S04-T01: Layout i8..i64, u8..u64 → correct size/align
S04-T02: Layout f32, f64 → 4/8 bytes
S04-T03: Layout bool, char, (), ! → correct
S04-T04: Layout &T, *const T → pointer_size
S04-T05: Layout [T] → LayoutError::Unsized
S04-T06: Layout alignment > ALIGN_MAX → LayoutError
S04-T07: fn_abi_of basic signature → correct ArgAbi
```
**DoD:** `SimpleLayoutComputer` handles all primitives. Zero STUBs.

### S05: Unification
**Mocking:** Use `TyCtxMut::new(Interner::new())`.
**Tests:**
```
S05-T01: Unify i32 with i32 → Ok
S05-T02: Unify i32 with u32 → Err
S05-T03: Unify TyVar with concrete type → binds
S05-T04: Unify IntVar with i32 → binds
S05-T05: Unify IntVar with bool → Err (kind mismatch)
S05-T06: Unify FloatVar with f64 → binds
S05-T07: Unify FloatVar with i32 → Err
S05-T08: Unify &mut T with &T → Err (mutability)
S05-T09: Unify error type with anything → Ok
S05-T10: fully_resolve returns Err for unresolved IntVar (F1 fix)
S05-T11: resolve_ty_shallow follows transitive bindings
```
**DoD:** `fully_resolve` collects Ty, Int, Float vars. `unify_tys` handles all `InferVar` kinds.

### S06: MIR-Interpreter
**Mocking:** Hand-craft `Body` and `BasicBlockData`.
**Tests:**
```
S06-T01: Interpret integer add → 3 + 4 = 7
S06-T02: Interpret branch true → Goto then_bb
S06-T03: Interpret branch false → Goto else_bb
S06-T04: Interpret function call → params + return
S06-T05: Interpret infinite loop → TimedOut
S06-T06: Interpret deep recursion → StackOverflow
S06-T07: Unreachable terminator → Panic with backtrace
S06-T08: Allocation write + read → correct value
```
**DoD:** Executes basic arithmetic, control flow, and calls.

### S07: Bytecode-Backend
**Mocking:** Hand-craft `glyim_mir::Body`.
**Tests:**
```
S07-T01: Lower empty function → Return opcode
S07-T02: Lower function with constants → LoadConst + Add + Return
S07-T03: Lower branch → JumpIf + Jump
S07-T04: generate() produces BytecodeModule
S07-T05: generate_function() produces Vec<u8>
```
**DoD:** Emits valid bytecode for simple MIR.

### S08: LLVM-Backend
**Mocking:** Hand-craft `glyim_mir::Body`.
**Tests:**
```
S08-T01: Create module without crash
S08-T02: Create function declaration in module
S08-T03: Create basic blocks in function
S08-T04: generate() returns CodegenResult
S08-T05: Context reuse across multiple generate() calls
```
**DoD:** `LlvmBackend` generates IR stubs without crashing.

### S09: Parser
**Mocking:** Use `glyim-lexer` (if S01 done) or mock token stream.
**Tests:**
```
S09-T01: Parse all item declarations → FnDef, StructDef, EnumDef, etc.
S09-T02: Parse expression precedence → 1 + 2 * 3 = Binary(Add, 1, Binary(Mul, 2, 3))
S09-T03: Parse method calls & field access → foo.bar(), foo.baz
S09-T04: Parse full pattern grammar → wild, binding, struct, tuple
S09-T05: Parse type grammar → path, ref, slice, array, fn, never
S09-T06: Error recovery: missing semicolon → error + continue
S09-T07: Error recovery: mismatched braces → error + sync
S09-T08: Idempotency: no token loss → parse covers all tokens
```
**DoD:** Complete parser for v0.1.0 grammar. Error recovery works.

### S10: TypeDisplay-Flags
**Mocking:** Implement `TypeLookup` for a mock struct.
**Tests:**
```
S10-T01: PrintTy renders all primitives → "i32", "bool", "()"
S10-T02: PrintTy renders compound → "&mut i32", "[T]", "(A, B)"
S10-T03: PrintTy recursion limit → deeply nested → "…"
S10-T04: compute_flags detects HAS_TY_INFER
S10-T05: compute_flags detects HAS_ERROR
S10-T06: compute_flags HAS_DEPTH_OVERFLOW ≠ HAS_ERROR
S10-T07: ty_is_error only checks HAS_ERROR
```
**DoD:** `PrintTy` handles all `TyKind` variants. `compute_flags` is generic and complete.

### S11: TraitSolver
**Mocking:** Use `glyim_solve::TraitContext`.
**Tests:**
```
S11-T01: Prove trait with matching impl → Proven
S11-T02: Prove trait with no impl → Ambiguous
S11-T03: BFS obligation processing → correct order
S11-T04: Overflow protection → limit exceeded → Err
S11-T05: Multiple obligations processed → all checked
```
**DoD:** `SimpleTraitSolver` works. `FulfillmentCtx` is robust.

### S12: HIR-Lowering
**Mocking:** Mock `SyntaxNode` tree or use S09 output.
**Tests:**
```
S12-T01: fn item → FnItem with params
S12-T02: struct item → StructItem with fields
S12-T03: Block expr → Body with exprs/pats
S12-T04: Binary expr → Expr::Binary
S12-T05: Path expr → Expr::Path with PathKind
S12-T06: Type refs → TypeRef roundtrip
```
**DoD:** Produces populated `CrateHir` from CST.

### S13: DefMap
**Mocking:** Use `glyim_core::path::Path` and mock CST.
**Tests:**
```
S13-T01: Empty file → root module, no items
S13-T02: Single fn → appears in scope
S13-T03: Inline module → child module
S13-T04: Visibility: pub vs private
S13-T05: Path resolution: plain, self::, super::, crate::
S13-T06: Duplicate name → error diagnostic
```
**DoD:** `Resolver::resolve_path` works for basic paths.

### S14: Typeck-Driver
**Mocking:** Hand-craft `CrateHir` and `CrateDefMap`.
**Tests:**
```
S14-T01: Typecheck empty crate → no errors
S14-T02: Typecheck fn returning unit → Ok
S14-T03: Typecheck i32 + i32 → i32
S14-T04: Typecheck i32 + bool → error diagnostic
S14-T05: Typecheck &x and &mut x
S14-T06: Inference: let x = 42 → i32
S14-T07: Obligation collection and fulfillment
```
**DoD:** `typeck_crate` produces `TypeckResult` with `expr_types` and `thir_bodies`.

### S15: MIR-Lowering
**Mocking:** Hand-craft `thir::Body` (using `glyim-type` sentinels).
**Tests:**
```
S15-T01: Lower empty function → Return terminator
S15-T02: Lower params → locals allocated
S15-T03: Lower let binding → StorageLive + Assign
S15-T04: Lower binary op → BinaryOp rvalue
S15-T05: Lower if-else → SwitchInt
S15-T06: Lower function call → Call terminator
```
**DoD:** Produces `Body` with real basic blocks.

### S16: Borrowck
**Mocking:** Hand-craft `glyim_mir::Body` with borrows.
**Tests:**
```
S16-T01: No borrows → no errors
S16-T02: Two shared borrows → no error
S16-T03: Shared + mutable → error
S16-T04: Use after move → error
S16-T05: Region constraints extracted
```
**DoD:** Detects basic borrow conflicts.

### S17: MIR-Opt
**Mocking:** Hand-craft `Body` with optimization opportunities.
**Tests:**
```
S17-T01: Constant prop: let x = 5; y = x + 3 → y = 8
S17-T02: DCE: unused assignment removed
S17-T03: CFG simplification: merge single-successor blocks
S17-T04: No-op pass: body unchanged
```
**DoD:** 3 optimization passes functional.

### S18: Pipeline
**Mocking:** Real files on disk.
**Tests:**
```
S18-T01: Compile empty file → Ok
S18-T02: Compile "fn main() {}" → Ok
S18-T03: Compile type error → diagnostic
S18-T04: Compile syntax error → diagnostic
S18-T05: Missing file → I/O error
```
**DoD:** End-to-end compilation pipeline runs.

### S19: LSP
**Mocking:** `Database` instance.
**Tests:**
```
S19-T01: did_open → file in VFS
S19-T02: diagnostics for error file → LspDiagnostic
S19-T03: Unix URI roundtrip
S19-T04: Windows URI roundtrip
S19-T05: Byte offset to position
```
**DoD:** LSP handles basic file operations.

### S20: CLI
**Mocking:** None.
**Tests:**
```
S20-T01: Compile valid file → exit 0
S20-T02: Compile invalid file → exit 1 + diagnostics
S20-T03: --backend bytecode → runs
S20-T04: --help → prints usage
```
**DoD:** CLI works end-to-end.

---

## 4. Dispatch Protocol for AI Agents

**Agent Setup:**
Every agent gets:
1. The locked `contracts-locked-v0.1.0` tag.
2. The specific Stream TDD plan above.
3. A strict rule: **You may NOT modify any `pub` interface in any crate you do not own.**

**Merge Rules:**
- Agents create PRs to `main`.
- CI runs `cargo test --workspace` and `cargo clippy --workspace`.
- If an agent changes a `pub` signature → **Auto-reject PR**.
- If an agent introduces a `todo!` in non-test code → **Auto-reject PR**.
- If an agent breaks another stream's tests → **Block PR** until fixed.

---

## 5. Timeline & Parallelism Matrix

| Time (Days) | S01 | S02 | S03 | S04 | S05 | S06 | S07 | S08 | S09 | S10 | S11 | S12 | S13 | S14 | S15 | S16 | S17 | S18 | S19 | S20 |
|-------------|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|-----|
| Day 1-3     | ██  | ██  | ██  | ██  | ██  | ██  | ██  | ██ |     |     |     |     |     |     |     |     |     |     |     |     |
| Day 4-6     |     |     |     |     |     |     |     |     | ██  | ██  | ██  | ██  |     |     |     |     |     |     |     |     |
| Day 7-10    |     |     |     |     |     |     |     |     |     |     |     |     | ██  | ██  | ██  | ██  | ██  |     |     |     |
| Day 11-13   |     |     |     |     |     |     |     |     |     |     |     |     |     |     |     |     |     | ██  | ██  | ██  |

**Total Time: ~13 Days with 8 agents running in parallel.** (vs 30+ days sequential).
