# Glyim v0.1.0 — TDD Parallel Dispatch: Organization & Verification Plan

## Step 1: Current State Audit

I've analyzed every file. Here's what's actually implemented vs stubbed:

| Crate | Status | What's Missing |
|-------|--------|----------------|
| `glyim-core` | **Done** | Complete: arena, primitives, def_id, abi, interner, path |
| `glyim-span` | **Done** | Complete: FileId, ByteIdx, Span, hygiene, MultiSpan |
| `glyim-diag` | **Done** | Complete: GlyimDiagnostic, DiagSink, miette impl |
| `glyim-vfs` | **Done** | Complete: VFS with RwLock, add_file, file_content |
| `glyim-syntax` | **Done** | Complete: SyntaxKind with num_enum, Rowan integration |
| `glyim-type` | **70%** | Structure exists; `compute_flags` has gaps; `PrintTy` has catch-all; `mk_*` methods work but many `TyKind` variants unhandled |
| `glyim-frontend` | **5%** | `lex()` returns empty; `parse_to_syntax()` returns empty SourceFile |
| `glyim-def-map` | **5%** | `build_def_map` returns empty |
| `glyim-hir` | **30%** | Types defined; no CST→HIR lowering |
| `glyim-mir` | **40%** | Types defined; `Place::ty()` missing; no real operations |
| `glyim-layout` | **5%** | `SimpleLayoutComputer` is stub |
| `glyim-solve` | **5%** | `InferenceTable` is empty struct; `TraitSolver` is stub |
| `glyim-typeck` | **5%** | `typeck_crate` just freezes |
| `glyim-lower` | **5%** | `lower_body` returns dummy; `MonoCtx` is empty |
| `glyim-borrowck` | **5%** | `check_borrows` returns empty |
| `glyim-opt` | **0%** | `optimize` panics |
| `glyim-mir-interp` | **0%** | Empty file |
| `glyim-codegen` | **10%** | Trait defined; `BytecodeBackend` returns empty |
| `glyim-codegen-llvm` | **5%** | `LlvmBackend` returns empty |
| `glyim-db` | **30%** | Database struct exists; no Salsa |
| `glyim-pipeline` | **5%** | `compile_file` does nothing |
| `glyim-cli` | **10%** | Args parsed; `run()` returns Ok |
| `glyim-lsp` | **0%** | Empty file |
| `glyim-runtime` | **15%** | FFI stubs exist (unsafe) |
| `glyim-test` | **10%** | `test_ty_ctx` works; nothing else |

---

## Step 2: The 20 Streams (Maximum Granularity)

Each stream is scoped to **one crate or one cohesive module**. Agents can test independently by mocking upstream contracts.

### Wave 1: Zero Upstream Dependencies (8 streams, launch immediately)

| ID | Stream Name | Crate(s) | Scope | Est. Tests |
|----|------------|----------|-------|-----------|
| S01 | Lexer | `glyim-frontend::lexer` | Complete lexer: keywords, literals, operators, error recovery | 15 |
| S02 | TypeInterning | `glyim-type::context` | `TyCtxMut` construction, `mk_*` methods, substitution interning, freeze | 12 |
| S03 | MIRCore | `glyim-mir` | `Place::ty()`, `Body::dummy`, `Statement`/`Terminator` construction helpers | 10 |
| S04 | LayoutEngine | `glyim-layout` | `SimpleLayoutComputer` for all primitives, reject unsized | 8 |
| S05 | Unification | `glyim-solve::infer` | `InferenceTable` with full unification, resolution | 14 |
| S06 | MIRInterpreter | `glyim-mir-interp` | Execute arithmetic, control flow, function calls | 10 |
| S07 | BytecodeBackend | `glyim-codegen` | Emit opcodes from hand-crafted MIR | 6 |
| S08 | LLVMBackend | `glyim-codegen-llvm` | Generate IR from hand-crafted MIR | 5 |

### Wave 2: Mild Upstream Dependencies (4 streams, can start with mocks)

| ID | Stream Name | Crate(s) | Scope | Est. Tests |
|----|------------|----------|-------|-----------|
| S09 | Parser | `glyim-frontend::parser` | Full recursive-descent parser for v0.1.0 grammar | 12 |
| S10 | TypeDisplay | `glyim-type::display` + `flags` | `PrintTy` for all variants; `compute_flags` complete | 9 |
| S11 | TraitSolver | `glyim-solve::solver` + `fulfill` | `SimpleTraitSolver`, `FulfillmentCtx` with BFS | 7 |
| S12 | HIRLowering | `glyim-hir` | CST→HIR conversion (add `lower_from_cst` function) | 10 |

### Wave 3: Heavy Dependencies (5 streams, must mock upstream structs)

| ID | Stream Name | Crate(s) | Scope | Est. Tests |
|----|------------|----------|-------|-----------|
| S13 | DefMap | `glyim-def-map` | Path resolution, scope population | 8 |
| S14 | TypeckDriver | `glyim-typeck` | `typeck_crate` with real inference and THIR construction | 10 |
| S15 | MIRLowering | `glyim-lower::lower` | THIR→MIR (hand-craft THIR for tests) | 8 |
| S16 | Borrowck | `glyim-borrowck` | Extract constraints, check conflicts (hand-craft MIR) | 7 |
| S17 | MIR opt | `glyim-opt` | Constant propagation, DCE, CFG simplification | 5 |

### Wave 4: Integration (3 streams, wire everything together)

| ID | Stream Name | Crate(s) | Scope | Est. Tests |
|----|------------|----------|-------|-----------|
| S18 | Pipeline | `glyim-pipeline` + `glyim-db` | `compile_file` driving all phases | 6 |
| S19 | LSP | `glyim-lsp` | `did_open`, `did_change`, diagnostics, URI handling | 7 |
| S20 | CLI | `glyim-cli` | Argument parsing, backend selection, miette rendering | 5 |

---

## Step 3: TDD Plan Per Stream (Detailed Test Cases)

Below is the **complete test specification** for each stream. This is what you'll give to the AI agents.

---

### S01: Lexer

**Locked interfaces:** `glyim_syntax::SyntaxKind`, `glyim_span::{FileId, Span, ByteIdx}`, `glyim_diag::GlyimDiagnostic`

**Mocking needed:** None. Pure function: `&str + FileId → LexResult`

```
S01-T01: Lex all keywords (30+) → each produces correct SyntaxKind
S01-T02: Lex decimal integers → "0", "42", "999999"
S01-T03: Lex hex integers → "0xFF", "0x0", "0xDEAD"
S01-T04: Lex binary integers → "0b0", "0b1010", "0b1"
S01-T05: Lex octal integers → "0o0", "0o77", "0o123"
S01-T06: Lex integer suffixes → "42u8", "0xFFi32", "100isize"
S01-T07: Lex float literals → "3.14", "1.0", "0.5"
S01-T08: Lex float exponents → "1e10", "1.5e-3", "2E+5"
S01-T09: Lex float error: "1e" → error diagnostic, IntLit for "1"
S01-T10: Lex float error: "1e+" → error diagnostic, IntLit for "1"
S01-T11: Lex string literals → r#""hello""#, r#""with \"escape\"""#
S01-T12: Lex char literals → "'a'", "'\\n'", "'\\x41'"
S01-T13: Lex all operators → "+", "-", "*", "/", "%", "==", "!=", "<=", ">="
S01-T14: Lex compound operators → "+=", "-=", "*=", "/=", "::", "->", "=>"
S01-T15: Lex nested block comments → "/* outer /* inner */ still outer */"
S01-T16: Lex unexpected character → error diagnostic with span
S01-T17: Token spans are contiguous and cover entire source
S01-T18: Empty input → empty token list, no diagnostics
```

**DoD:** `cargo test -p glyim-frontend --test lexer` passes. Zero `todo!` in lexer code.

---

### S02: TypeInterning

**Locked interfaces:** All of `glyim_type` public API. `glyim_core::interner::Interner`.

**Mocking needed:** None. `TyCtxMut::new(Interner::new())` is standalone.

```
S02-T01: Sentinel constants at correct indices → Ty::ERROR.index() == 0, NEVER == 1, UNIT == 2, BOOL == 3
S02-T02: freeze() verifies sentinels → manually corrupt types[0], freeze panics
S02-T03: mk_ref creates Ref type → construct &mut i32, inspect via ty_kind
S02-T04: mk_ref with Not mutability → construct &i32
S02-T05: mk_adt with empty Substitution → verify roundtrip
S02-T06: mk_adt with Substitution containing Ty args → args roundtrip via substitution_args
S02-T07: mk_tuple with multiple tys → verify via substitution_args
S02-T08: mk_fn_ptr with FnSig → signature roundtrip
S02-T09: Substitution interning deduplicates → same args → same Substitution index
S02-T10: Substitution with 10 args → no panic, correct length
S02-T11: error_ty() returns Ty::ERROR sentinel
S02-T12: TyCtxMut is !Send + !Sync → compile_fail test
S02-T13: TyCtx is Send + Sync → compiles
S02-T14: Region variable allocation → new_region_var, set_region, retrieve
```

**DoD:** `cargo test -p glyim-type --test interning` passes.

---

### S03: MIRCore

**Locked interfaces:** `glyim_mir` public types, `glyim_type::TypeLookup`, `glyim_type::Ty`.

**Mocking needed:** Implement `TypeLookup` for a mock struct in tests.

```
S03-T01: Body::dummy creates valid structure → has Unreachable terminator, Ty::ERROR return type
S03-T02: Place::new creates place with empty projection
S03-T03: Place::ty() Deref on &T → returns T
S03-T04: Place::ty() Deref on &mut T → returns T
S03-T05: Place::ty() Deref on *const T → returns T
S03-T06: Place::ty() Deref on *mut T → returns T
S03-T07: Place::ty() Deref on non-pointer → returns Ty::ERROR + tracing::error
S03-T08: Place::ty() Field on Tuple → returns correct arg Ty
S03-T09: Place::ty() Field on non-tuple → returns Ty::ERROR
S03-T10: Place::ty() Index on [T; N] → returns T
S03-T11: Place::ty() Index on [T] → returns T
S03-T12: Place::ty() Index on non-array/slice → returns Ty::ERROR
S03-T13: Place::ty() Downcast returns same type
S03-T14: Place::ty() chained projections: &((i32, u32)).0 → i32
```

**DoD:** `Place::ty()` handles all `ProjectionElem` variants without cloning `TyKind`.

---

### S04: LayoutEngine

**Locked interfaces:** `glyim_layout` public types, `glyim_type::TyCtx`.

**Mocking needed:** Use `test_frozen_ty_ctx()` from `glyim-test`.

```
S04-T01: Layout i8 → Size(1), Align(1)
S04-T02: Layout i16 → Size(2), Align(2)
S04-T03: Layout i32 → Size(4), Align(4)
S04-T04: Layout i64 → Size(8), Align(8)
S04-T05: Layout f32 → Size(4), Align(4)
S04-T06: Layout f64 → Size(8), Align(8)
S04-T07: Layout bool → Size(1), Align(1)
S04-T08: Layout () → Size(0), Align(1)
S04-T09: Layout ! → Size(0), Align(1)
S04-T10: Layout &T → pointer_size, pointer_align
S04-T11: Layout *const T → pointer_size, pointer_align
S04-T12: Layout [T] → LayoutError::Unsized
S04-T13: Layout dyn Trait → LayoutError::Unsized
S04-T14: fn_abi_of basic signature → correct ArgAbi layout
```

**DoD:** `SimpleLayoutComputer` handles all primitives. Zero `unimplemented!`.

---

### S05: Unification

**Locked interfaces:** `glyim_solve::infer`, `glyim_type::*`.

**Mocking needed:** Use `TyCtxMut::new(Interner::new())`.

```
S05-T01: Unify i32 with i32 → Ok([])
S05-T02: Unify i32 with u32 → Err (mismatched types)
S05-T03: Unify TyVar with i32 → binds variable
S05-T04: Unify IntVar with i32 → binds variable
S05-T05: Unify IntVar with bool → Err (expected integer)
S05-T06: Unify IntVar with f64 → Err (expected integer)
S05-T07: Unify FloatVar with f64 → binds variable
S05-T08: Unify FloatVar with i32 → Err (expected float)
S05-T09: Unify &mut T with &T → Err (mutability mismatch)
S05-T10: Unify &T with &T where T:T → recursive unify
S05-T11: Unify error type with anything → Ok (error recovery)
S05-T12: resolve_ty_shallow follows one binding
S05-T13: resolve_ty_shallow follows transitive chain
S05-T14: fully_resolve returns Err for unresolved TyVar
S05-T15: fully_resolve returns Err for unresolved IntVar
S05-T16: fully_resolve returns Err for unresolved FloatVar
S05-T17: fully_resolve returns Ok for fully resolved type
S05-T18: New TyVar, IntVar, FloatVar each create distinct index types
```

**DoD:** `fully_resolve` collects all variable kinds. `unify_tys` enforces kind constraints.

---

### S06: MIRInterpreter

**Locked interfaces:** `glyim_mir` types.

**Mocking needed:** Hand-craft `Body` with `BasicBlockData`.

```
S06-T01: Interpret integer add → 3 + 4 = 7
S06-T02: Interpret integer sub → 10 - 3 = 7
S06-T03: Interpret branch on true → takes then_bb
S06-T04: Interpret branch on false → takes else_bb
S06-T05: Interpret function call → params passed, return value
S06-T06: Interpret infinite loop → TimedOut
S06-T07: Interpret deep recursion → StackOverflow
S06-T08: Unreachable terminator → Panic with backtrace
S06-T09: Allocate + write + read → correct value
S06-T10: Step limit default is 1_000_000
S06-T11: Recursion limit default is 256
```

**DoD:** Executes basic arithmetic, control flow, and function calls.

---

### S07: BytecodeBackend

**Locked interfaces:** `glyim_codegen::CodegenBackend`, `glyim_mir::Body`.

**Mocking needed:** Hand-craft `Body` with simple structure.

```
S07-T01: Empty function → produces module with Return opcode
S07-T02: Function with integer constants → LoadConst + Add + Return
S07-T03: Function with locals → LoadLocal + StoreLocal
S07-T04: Branch → JumpIf + Jump opcodes
S07-T05: generate() returns non-empty Vec<u8>
S07-T06: name() returns "bytecode"
```

**DoD:** Emits valid bytecode for simple MIR.

---

### S08: LLVMBackend

**Locked interfaces:** `glyim_codegen::CodegenBackend`, `glyim_mir::Body`.

**Mocking needed:** Hand-craft `Body` or use `Body::dummy`.

```
S08-T01: Create LlvmBackend without crash
S08-T02: generate() with empty bodies → creates module
S08-T03: generate() returns Ok
S08-T04: name() returns "llvm"
S08-T05: Multiple generate() calls reuse Context (no crash)
```

**DoD:** `LlvmBackend` generates IR stubs without crashing.

---

### S09: Parser

**Locked interfaces:** `glyim_syntax::*`, `glyim_frontend::lexer`.

**Mocking needed:** Uses real lexer (S01 output) or mock token stream.

```
S09-T01: Parse fn item → FnDef node with name, params, return type, body
S09-T02: Parse struct (unit, tuple, record) → StructDef with FieldList
S09-T03: Parse enum with variants → EnumDef with VariantList
S09-T04: Parse trait def → TraitDef node
S09-T05: Parse impl def → ImplDef node
S09-T06: Parse expression precedence: 1 + 2 * 3 → Binary(Add, 1, Binary(Mul, 2, 3))
S09-T07: Parse method call and field access → MethodCallExpr, FieldExpr
S09-T08: Parse pattern grammar → wild, binding, struct, tuple
S09-T09: Parse type grammar → path, ref, slice, array, tuple, fn, never
S09-T10: Error recovery: missing semicolon → error + continue parsing
S09-T11: Error recovery: mismatched braces → error + sync on closing brace
S09-T12: No token loss: parse covers all tokens from lex
S09-T13: Snapshot test: representative program CST
```

**DoD:** Complete parser for v0.1.0 grammar. Error recovery works.

---

### S10: TypeDisplay + Flags

**Locked interfaces:** `glyim_type::display::TypeLookup`, `glyim_type::flags`.

**Mocking needed:** Implement `TypeLookup` for a mock struct in tests.

```
S10-T01: PrintTy renders i32, u32, f64, bool, char, !, ()
S10-T02: PrintTy renders &mut i32, &i32, *const u8, *mut u8
S10-T03: PrintTy renders [T], (A, B), fn(A) -> B
S10-T04: PrintTy renders Adt with substitution
S10-T05: PrintTy recursion limit → deeply nested type prints "…"
S10-T06: compute_flags detects HAS_TY_INFER on type with Infer var
S10-T07: compute_flags detects HAS_ERROR on TyKind::Error
S10-T08: compute_flags sets HAS_DEPTH_OVERFLOW at depth > 64
S10-T09: HAS_DEPTH_OVERFLOW does NOT set HAS_ERROR
S10-T10: ty_is_error only checks HAS_ERROR, ignores HAS_DEPTH_OVERFLOW
S10-T11: compute_flags propagates flags through Ref, Slice, Array
S10-T12: compute_flags propagates flags through Substitution args
```

**DoD:** `PrintTy` handles all `TyKind` variants (no catch-all). `compute_flags` is exhaustive.

---

### S11: TraitSolver

**Locked interfaces:** `glyim_solve::solver`, `glyim_solve::fulfill`.

**Mocking needed:** Use `TraitContext` directly.

```
S11-T01: Register trait → appears in trait_defs
S11-T02: Register impl → appears in impl_defs
S11-T03: Prove trait with matching impl → Proven
S11-T04: Prove trait with no impl → Ambiguous
S11-T05: impls_of_trait returns correct subset
S11-T06: FulfillmentCtx registers obligations
S11-T07: BFS processing: obligations processed in order
S11-T08: Overflow protection: limit exceeded → Err(OverflowError)
S11-T09: Multiple obligations all checked
S11-T10: Ambiguous obligation produces warning diagnostic
S11-T11: Definite no produces error diagnostic
```

**DoD:** `SimpleTraitSolver` and `FulfillmentCtx` work end-to-end.

---

### S12: HIRLowering

**Locked interfaces:** `glyim_hir` types, `glyim_syntax::SyntaxNode`.

**Mocking needed:** Use real `parse_to_syntax()` or construct SyntaxNodes.

```
S12-T01: Fn item → FnItem with params, return type, body ID
S12-T02: Struct item (record) → StructItem with named fields
S12-T03: Struct item (unit) → StructItem with empty fields
S12-T04: Enum item → EnumItem with variants
S12-T05: Block expression → Body with exprs and pats
S12-T06: Binary expression → Expr::Binary with correct op
S12-T07: If/else expression → Expr::If with branches
S12-T08: Path expression → Expr::Path with PathKind
S12-T09: Literal expressions → Expr::Literal with correct types
S12-T10: Type references → TypeRef variants roundtrip
S12-T11: Pattern wild → Pat::Wild
S12-T12: Pattern binding → Pat::Binding with name
```

**DoD:** Produces populated `CrateHir` from CST.

---

### S13: DefMap

**Locked interfaces:** `glyim_def_map` types, `glyim_syntax::SyntaxNode`.

**Mocking needed:** Use real parse output.

```
S13-T01: Empty file → root module with no items
S13-T02: Single fn → appears in scope
S13-T03: Struct, enum, trait, impl → all appear in scope
S13-T04: Inline module → child module with items
S13-T05: Visibility: pub items visible to siblings
S13-T06: Path resolution: plain path (foo::bar)
S13-T07: Path resolution: self::foo
S13-T08: Path resolution: super::foo
S13-T09: Path resolution: crate::foo
S13-T10: Duplicate name → error diagnostic
S13-T11: Unknown name → PerNs::default()
```

**DoD:** `Resolver::resolve_path` works for basic paths.

---

### S14: TypeckDriver

**Locked interfaces:** `glyim_typeck` public API.

**Mocking needed:** Hand-craft `CrateHir` and `CrateDefMap`.

```
S14-T01: Typecheck empty crate → no errors
S14-T02: Typecheck fn returning () → Ok
S14-T03: Typecheck fn returning i32 → Ok (if body is int literal)
S14-T04: Typecheck i32 + i32 → result is i32
S14-T05: Typecheck i32 + bool → error diagnostic
S14-T06: Typecheck &x where x: i32 → &i32
S14-T07: Typecheck &mut x where x: i32 → &mut i32
S14-T08: Inference: let x = 42 → x: i32
S14-T09: Obligation collection → pending_obligations populated
S14-T10: Obligation fulfillment → processed by FulfillmentCtx
```

**DoD:** `typeck_crate` produces `TypeckResult` with `expr_types` populated.

---

### S15: MIRLowering

**Locked interfaces:** `glyim_lower` public API.

**Mocking needed:** Hand-craft `thir::Body` using `Ty::ERROR` sentinels.

```
S15-T01: Lower empty function → single block with Return
S15-T02: Lower function with params → locals for each param
S15-T03: Lower let binding → StorageLive + Assign
S15-T04: Lower return statement → Return terminator
S15-T05: Lower binary op → BinaryOp rvalue
S15-T06: Lower if-else → SwitchInt terminator
S15-T07: Lower function call → Call terminator
S15-T08: Lower reference expression → Ref rvalue
```

**DoD:** Produces `Body` with real basic blocks.

---

### S16: Borrowck

**Locked interfaces:** `glyim_borrowck` public API.

**Mocking needed:** Hand-craft `glyim_mir::Body` with known borrow patterns.

```
S16-T01: Function with no borrows → no errors
S16-T02: Single shared borrow → no error
S16-T03: Two shared borrows at same point → no error
S16-T04: Shared + mutable borrow at same point → error
S16-T05: Two mutable borrows at same point → error
S16-T06: Borrow expires after last use → no error
S16-T07: Region constraints extracted from Ref rvalues
S16-T08: Error diagnostics include span
```

**DoD:** Detects basic borrow conflicts.

---

### S17: MIR opt

**Locked interfaces:** `glyim_opt` public API.

**Mocking needed:** Hand-craft `Body` with optimization opportunities.

```
S17-T01: Constant propagation: x = 5; y = x + 3 → y replaced
S17-T02: Dead code elimination: unused assignment removed
S17-T03: CFG simplification: merge single-successor blocks
S17-T04: Unreachable block elimination
S17-T05: No-op pass: body unchanged when nothing to optimize
```

**DoD:** 3 optimization passes functional.

---

### S18: Pipeline

**Locked interfaces:** `glyim_pipeline`, `glyim_db`.

**Mocking needed:** Real files on disk.

```
S18-T01: Compile empty file → Ok
S18-T02: Compile "fn main() {}" → Ok
S18-T03: Compile type error → diagnostic
S18-T04: Compile syntax error → diagnostic
S18-T05: Missing input file → I/O error
S18-T06: Backend selection: bytecode runs
```

**DoD:** End-to-end compilation pipeline runs.

---

### S19: LSP

**Locked interfaces:** `glyim_lsp`, `glyim_db`.

**Mocking needed:** `Database` instance.

```
S19-T01: did_open registers file in VFS
S19-T02: did_open with content → file_content returns same content
S19-T03: URI conversion: Unix "file:///home/user/foo.rs" → "/home/user/foo.rs"
S19-T04: URI conversion: Windows "file:///C:/Users/foo.rs" → "C:/Users/foo.rs"
S19-T05: URI conversion: no scheme → pass through as-is
S19-T06: path_to_uri Unix roundtrip
S19-T07: Byte offset to line/column conversion
```

**DoD:** LSP handles basic file operations and URI conversion.

---

### S20: CLI

**Locked interfaces:** `glyim_cli`.

**Mocking needed:** None.

```
S20-T01: Compile valid file → exit 0
S20-T02: Compile invalid file → exit 1
S20-T03: --help → prints usage
S20-T04: Missing input arg → error
S20-T05: --backend bytecode → selects bytecode backend
```

**DoD:** CLI works end-to-end.

---

## Step 4: Coherence Verification Checklist

Before dispatching, verify these cross-stream contracts are consistent:

| # | Cross-Stream Contract | Verified? |
|---|----------------------|-----------|
| C01 | S01 Lexer output matches S09 Parser input (`Token` type) | ☐ |
| C02 | S09 Parser output matches S12 HIR input (`SyntaxNode`) | ☐ |
| C03 | S02 TyCtx output matches S05 Unification input (`TyCtxMut`) | ☐ |
| C04 | S05 Unification output matches S14 Typeck input (`InferenceTable`) | ☐ |
| C05 | S14 Typeck output (`thir::Body`) matches S15 MIR Lowering input | ☐ |
| C06 | S15 MIR output matches S16 Borrowck input (`glyim_mir::Body`) | ☐ |
| C07 | S15 MIR output matches S17 MIR Opt input | ☐ |
| C08 | S17 MIR Opt output matches S07/S08 Codegen input | ☐ |
| C09 | S10 TypeDisplay works with both S02 TyCtxMut and frozen TyCtx | ☐ |
| C10 | All streams use same `Ty::ERROR`/`Ty::NEVER`/`Ty::UNIT`/`Ty::BOOL` sentinels | ☐ |
| C11 | `GlyimDiagnostic` spans use same `FileId`/`Span` types everywhere | ☐ |
| C12 | `BorrowckCtx` trait in S16 matches `glyim_borrowck` contract | ☐ |
| C13 | `LowerCtx` trait in S15 matches `glyim_lower` contract | ☐ |
| C14 | `CodegenBackend` trait in S07/S08 matches `glyim_codegen` contract | ☐ |
| C15 | `TypeLookup` trait in S03/S10 matches `glyim_type::display` contract | ☐ |

**Verification process:**
1. For each contract, write a **compile-time test** that asserts the types line up
2. Example for C01: write a function `fn _lexer_parser_compat(tokens: Vec<glyim_frontend::Token>)` in `glyim-frontend/tests/compat.rs`
3. If it compiles, the contract is verified
4. Run this verification BEFORE dispatching any agent

---

## Step 5: Dispatch Protocol

### Agent Prompt Template

```markdown
# Mission: Stream [S01-S20] — [Stream Name]

## Scope
- You own: [crate names and modules]
- You may NOT modify: [list of locked public interfaces]
- Your tests go in: [test directory]

## Test-First Rule
1. Write ALL test cases from the TDD subplan FIRST
2. Verify they compile (they will fail at runtime)
3. Implement until all tests pass
4. Run `cargo test -p [crate]` — all must pass
5. Run `cargo clippy -p [crate]` — zero warnings
6. Run `cargo fmt` — applied

## Interface Stability
- Do NOT change any `pub` signature
- Do NOT add new `pub` items to existing modules
- You MAY add `pub(crate)` helpers
- You MAY add new private modules

## Mocking Rules
- [Stream-specific mocking instructions]

## Definition of Done
- All TDD test cases pass
- Zero `todo!` in non-test code
- Zero clippy warnings
- No regressions: `cargo check --workspace` succeeds

## Output
- Git branch: `stream-S[XX]/v0.1.0`
- PR with test results
```

### Integration Checkpoints

| CP | After Streams | Gate | Action |
|----|--------------|------|--------|
| CP1 | S01-S08 | `cargo test -p glyim-frontend -p glyim-type -p glyim-mir -p glyim-layout -p glyim-solve -p glyim-mir-interp -p glyim-codegen -p glyim-codegen-llvm` | Merge |
| CP2 | S09-S12 | `cargo test -p glyim-frontend -p glyim-hir` | Merge |
| CP3 | S13-S17 | `cargo test -p glyim-def-map -p glyim-typeck -p glyim-lower -p glyim-borrowck -p glyim-opt` | Merge |
| CP4 | S18-S20 | `cargo test --workspace` | Release v0.1.0 |

---

## Step 6: Timeline

| Days | Active Streams | Agents Needed |
|------|---------------|---------------|
| 1-3 | S01, S02, S03, S04, S05, S06, S07, S08 | 8 |
| 4-6 | S09, S10, S11, S12 | 4 |
| 7-10 | S13, S14, S15, S16, S17 | 5 |
| 11-13 | S18, S19, S20 | 3 |

**Total: 13 days with 8 parallel agents** (vs 30+ days sequential).

---

## Next Step

Run the **Coherence Verification Checklist** (Step 4). Write the compile-time compatibility tests. Then dispatch Wave 1.
