# Glyim Agent Dispatch Kit — Templates & Document System

The goal: you create **5 files once**, then for each stream you run **one generator command** that produces a complete agent brief. No manual writing per stream.

---

## File 1: `AGENT_MASTER_CONTEXT.md`

This file is **included verbatim** in every agent prompt. It contains project-wide rules that never change.

```markdown
# Glyim Compiler — Master Agent Context

## Project Overview
Glyim is a from-scratch compiler for a Rust-like language, written in Rust. The codebase uses 25 crates organized in layers. You are implementing one stream of work within this project.

## Architecture Rules (NON-NEGOTIABLE)
1. **No `pub` signature changes.** If a public type or function exists in a crate you don't own, you MAY NOT modify it. If you need a change, add a `pub(crate)` helper instead.
2. **No new `pub` items in existing modules** without explicit approval. You MAY add new private modules and `pub(crate)` items freely.
3. **No `unsafe` in compiler crates.** Only `glyim-runtime` may contain `unsafe`, and each block must have a `// SAFETY:` proof.
4. **No `todo!()` in non-test code.** Use `tracing::warn!("STUB: {reason}")` for optional paths that need attention but won't crash.
5. **All stubs must be visible.** Silent no-ops (empty match arms, `let _ = x`) are forbidden in implementation code. Every stub must emit a warning on first execution.
6. **Tracing convention:** `trace` for hot paths, `debug` for inference, `info` for phases. Always `skip(self, ctx)`.
7. **Test-first:** Write all test cases from your stream's TDD plan BEFORE implementing. Tests must compile before implementation begins.

## Crate Dependency Rules
- Frontend crates (`glyim-syntax`, `glyim-frontend`, `glyim-hir`, `glyim-def-map`) NEVER depend on `glyim-type`.
- `glyim-lsp` depends ONLY on `glyim-db` (no LLVM transitive dependency).
- `glyim-db` does NOT use Salsa (removed for v0.1.0 honesty).
- `TyCtxMut` is `!Send + !Sync`. Post-typeck, only `TyCtx` (frozen, `Send + Sync`) is used.

## Key Type Contracts
- `Ty` does NOT implement `IdxLike`. Construction via `Ty::from_raw()` is `pub(crate)`. Use sentinels: `Ty::ERROR`, `Ty::NEVER`, `Ty::UNIT`, `Ty::BOOL`.
- `TypeLookup` trait bridges `TyCtx` and `TyCtxMut` for display and flag computation.
- `InferVar` has separate index types: `TyVar`, `IntVar`, `FloatVar`. Cross-kind construction is impossible by type.
- `compute_flags` is generic over `TypeLookup`: `fn compute_flags<L: TypeLookup>(kind: &TyKind, ctx: &L, depth: u32) -> TypeFlags`.
- `Substitution` is interned as `(u32 index, u16 len)`. Access via `ctx.substitution_args(sub)`.

## Error Handling
- Use `GlyimDiagnostic` for all errors. Constructors: `lex_error`, `parse_error`, `type_error`, `borrow_error`, `internal_error`.
- `DiagSink` collects diagnostics with error limiting (default 50).
- `CompResult<T> = Result<T, Vec<GlyimDiagnostic>>`.

## Git Convention
- Branch: `stream-S{XX}/v0.1.0` (e.g., `stream-S01/v0.1.0`)
- Commit format: `stream-S{XX}: description`
- Do NOT commit to `main` directly. Create a PR.
```

---

## File 2: `CONTRACTS_LOCKED.md`

Auto-generated from your codebase. Lists every `pub` item per crate. Agents reference this to know what they cannot touch.

```markdown
# Locked Public Contracts — v0.1.0

Generated from: `contracts-locked-v0.1.0` tag
Any change to items listed here requires a formal Change Request.

## glyim-core
- `pub struct Idx<T>` — `from_raw`, `to_raw`, `index`
- `pub trait IdxLike` — `from_raw`, `to_raw`, `index`
- `pub macro define_idx`
- `pub struct IndexVec<I, T>` — all methods
- `pub struct Name` — `as_symbol`
- `pub struct Interner` — `new`, `intern`, `resolve`, `lookup`
- `pub struct PathKind` — `Plain`, `SelfPath`, `Super(u32)`, `Crate`
- `pub struct PathSegment` — `name`
- `pub struct Path` — `from_single`, `as_name`, `segments`, `kind`
- `pub struct DefId` — `new`, `krate`, `local_id`
- `pub struct CrateId` — `from_raw`, `to_raw`
- `pub enum IntTy` — all variants and methods
- `pub enum UintTy` — all variants and methods
- `pub enum FloatTy` — all variants and methods
- `pub enum Mutability` — `is_mut`, `prefix_str`
- `pub enum BinOp` — `is_comparison`
- `pub enum UnOp`
- `pub enum Visibility`
- `pub enum StructKind`
- `pub fn validate_alignment`
- `pub const ALIGN_MAX`, `ALIGN_MIN`, `DEFAULT_STACK_SIZE`

## glyim-span
- `pub struct FileId` — `BOGUS`, `from_raw`, `to_raw`, `index`
- `pub struct ByteIdx` — `ZERO`, `from_raw`, `to_raw`, `to_usize`
- `pub struct Span` — `DUMMY`, `new`, `is_dummy`, `range`, `sans_ctx`, `len`, `to`
- `pub struct SyntaxContext` — `ROOT`, `is_root`, `to_raw`
- `pub struct ExpnId` — `ROOT`, `is_root`, `to_raw`
- `pub struct HygieneKey` — (no pub constructors)
- `pub struct MultiSpan` — `from_span`, `with_secondary`
- `pub struct HygieneCtx` — `new`, `push_expansion`, `apply_mark`, `remove_mark`, `expn_data`, `adjust`

## glyim-diag
- `pub struct GlyimDiagnostic` — all constructors, `with_source_code`, `with_sub`, `with_suggestion`, `is_error`
- `pub struct DiagSink` — `new`, `with_error_limit`, `emit`, `has_errors`, `into_diagnostics`
- `pub type CompResult<T>`
- `pub struct ErrorCode`, `ErrorCategory`
- `pub enum DiagSeverity`

## glyim-syntax
- `pub enum SyntaxKind` — all variants, `is_trivia`, `is_keyword`, `is_literal`, `is_node`, `try_from_raw`
- `pub enum GlyimLang` — Rowan language impl
- `pub type SyntaxNode`, `SyntaxToken`, `SyntaxElement`, `GreenNode`, `GreenToken`
- `pub trait AstNode` — `can_cast`, `cast`, `syntax`
- `pub fn child_of_kind`
- AST node types: `SourceFile`, `FnDef`, `StructDef`, `EnumDef`, `TraitDef`, `ImplDef`, `Block`, `CallExpr`, `BinaryExpr`, `PathExpr`, `LitExpr`

## glyim-type
- `pub struct Ty` — `to_raw`, `index`, `ERROR`, `NEVER`, `UNIT`, `BOOL`
- `pub enum TyKind` — all variants
- `pub enum InferVar` — `Ty(TyVar)`, `Int(IntVar)`, `Float(FloatVar)`
- `pub struct TyVar`, `IntVar`, `FloatVar`, `RegionVid`, `ConstVar`, `FieldIdx`
- `pub struct Substitution` — `index`, `len`, `is_empty`
- `pub enum GenericArg` — `Ty`, `Lifetime`, `Const`
- `pub enum Region` — all variants
- `pub struct FnSig`
- `pub enum Predicate`, `TraitPredicate`, `TraitRef`
- `pub struct Binder<T>` — `bind`, `skip_binder`, `as_ref`
- `pub struct TypeFlags` — all flag constants
- `pub fn compute_flags<L: TypeLookup>`
- `pub trait TypeLookup` — `ty_kind`, `ty_flags`, `substitution_args`, `name_str`, `error_ty`
- `pub struct PrintTy<'a, L>` — `new`
- `pub struct TyCtxMut` — `new`, `alloc_ty`, `mk_*`, `freeze`, `ty_kind`, `ty_flags`, `substitution_args`, `error_ty`
- `pub struct TyCtx` — `ty_kind`, `ty_flags`, `substitution_args`, `ty_is_error`, `ty_has_depth_overflow`, `error_ty`, `never_ty`, `unit_ty`, `bool_ty`

## glyim-mir
- `pub struct Body` — `dummy`, `owner`, `basic_blocks`, `locals`, `arg_count`, `return_ty`
- `pub struct Place` — `new`, `local`, `projection`
- `pub enum ProjectionElem` — `Deref`, `Field`, `Index`, `Downcast`
- `pub struct LocalDecl` — `ty`, `mutability`, `source_info`
- `pub enum StatementKind` — all variants
- `pub enum Rvalue` — all variants
- `pub enum TerminatorKind` — all variants
- `pub struct BasicBlockData` — `new`

## glyim-codegen
- `pub trait CodegenBackend` — `name`, `generate`
- `pub struct BytecodeBackend` — `new`

## glyim-codegen-llvm
- `pub struct LlvmBackend` — `new`

## glyim-db
- `pub struct Database` — `new`, `interner`, `vfs`

## glyim-pipeline
- `pub struct Pipeline` — `compile_file`

## glyim-frontend
- `pub fn lex(source: &str, file_id: FileId) -> LexResult`
- `pub fn parse_to_syntax(source: &str, file_id: FileId) -> ParseResult`
- `pub struct Token` — `kind`, `span`, `text`
- `pub struct LexResult` — `tokens`, `diagnostics`
- `pub struct ParseResult` — `green_node`, `diagnostics`, `root`

## glyim-def-map
- `pub struct CrateDefMap` — `root`, `modules`, `krate`
- `pub fn build_def_map(root: &SyntaxNode, krate: CrateId) -> (CrateDefMap, Vec<GlyimDiagnostic>)`

## glyim-hir
- `pub struct CrateHir` — `items`, `bodies`, `body_owners`
- All HIR types: `Item`, `ItemKind`, `FnItem`, `StructItem`, `EnumItem`, `Body`, `Expr`, `Pat`, `TypeRef`, `Path`

## glyim-solve
- `pub struct InferenceTable` — `new`
- `pub trait TraitSolver`
- `pub struct SimpleTraitSolver`
- `pub struct TraitContext`
- `pub struct FulfillmentCtx`

## glyim-typeck
- `pub fn typeck_crate(ctx: TyCtxMut, def_map: &CrateDefMap, hir: &CrateHir, solver: &mut dyn TraitSolver) -> (TyCtx, TypeckResult)`

## glyim-lower
- `pub trait LowerCtx`
- `pub fn lower_body(ctx: &dyn LowerCtx, thir: &ThirBody) -> LowerResult`

## glyim-borrowck
- `pub trait BorrowckCtx`
- `pub fn check_borrows(ctx: &dyn BorrowckCtx, body: &Body) -> BorrowckResult`

## glyim-opt
- `pub fn optimize(ctx: &TyCtx, body: &Arc<Body>) -> Optimized`

## glyim-layout
- `pub struct SimpleLayoutComputer<'a>` — `new`

## glyim-vfs
- `pub struct Vfs` — `new`, `add_file_from_disk`, `add_file_content`, `file_content`, `file_id`

## glyim-runtime
- `pub fn glyim_alloc`, `glyim_dealloc`, `glyim_panic`
- `pub use ALIGN_MAX`
```

---

## File 3: `stream-template.md`

The template for each stream brief. You fill in the `{VARIABLES}` once per stream.

```markdown
# Stream S{ID}: {NAME}

## Mission
Implement {SCOPE_SUMMARY} for the Glyim compiler.

## What You Own
{OWNED_CRATES_AND_MODULES}

## What You May NOT Modify
Any `pub` interface listed in `CONTRACTS_LOCKED.md` for crates you do NOT own.
Specifically, you MUST NOT change:
{LOCKED_INTERFACE_LIST}

## What You May Do Freely
- Add `pub(crate)` helper functions in crates you own
- Add new private modules in crates you own
- Add new `pub` items in NEW modules you create within crates you own
- Add dev-dependencies for testing

## Test Plan (Write These FIRST)
{TEST_CASES}

## Mocking Strategy
{MOCKING_INSTRUCTIONS}

## Implementation Priority
1. Write all tests above — verify they compile
2. Implement until all tests pass
3. Remove all `todo!()` and silent stubs from non-test code
4. Run `cargo clippy -p {CRATE_NAME}` — fix all warnings
5. Run `cargo fmt`

## Verification Commands
```bash
cargo test -p {CRATE_NAME}
cargo clippy -p {CRATE_NAME} -- -D warnings
cargo fmt --check -p {CRATE_NAME}
cargo check --workspace  # must not break other crates
```

## Definition of Done
- [ ] All test cases pass
- [ ] Zero `todo!()` in non-test code
- [ ] Zero clippy warnings
- [ ] `cargo fmt` applied
- [ ] `cargo check --workspace` succeeds
- [ ] PR created against `main` with branch `stream-S{ID}/v0.1.0`

## Dependencies on Other Streams
{UPSTREAM_DEPENDENCIES}

## Downstream Consumers
{WHO_USES_YOUR_OUTPUT}
```

---

## File 4: `generate-stream.sh`

A shell script that generates each stream brief from the template + a JSON data file. Run once per stream.

```bash
#!/usr/bin/env bash
set -euo pipefail

# Usage: ./generate-stream.sh S01
# Reads stream data from streams.json, produces briefs/S01.md

STREAM_ID="${1:?Usage: generate-stream.sh SXX}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DATA_FILE="$SCRIPT_DIR/streams.json"
TEMPLATE="$SCRIPT_DIR/stream-template.md"
OUTPUT_DIR="$SCRIPT_DIR/briefs"
mkdir -p "$OUTPUT_DIR"

if ! command -v jq &>/dev/null; then
    echo "jq is required. Install it first."
    exit 1
fi

# Extract stream data from JSON
STREAM_DATA=$(jq --arg id "$STREAM_ID" '.[] | select(.id == $id)' "$DATA_FILE")

if [ -z "$STREAM_DATA" ]; then
    echo "Stream $STREAM_ID not found in $DATA_FILE"
    exit 1
fi

# Read fields
NAME=$(echo "$STREAM_DATA" | jq -r '.name')
CRATE=$(echo "$STREAM_DATA" | jq -r '.crate')
SCOPE=$(echo "$STREAM_DATA" | jq -r '.scope_summary')
OWNED=$(echo "$STREAM_DATA" | jq -r '.owned_crates[]' | tr '\n' '\n' | sed 's/^/- /')
LOCKED=$(echo "$STREAM_DATA" | jq -r '.locked_interfaces[]' | tr '\n' '\n' | sed 's/^/- /')
TESTS=$(echo "$STREAM_DATA" | jq -r '.tests[]' | sed 's/^/```\n/' | sed 's/$/```/' | head -1) # simplified
MOCKING=$(echo "$STREAM_DATA" | jq -r '.mocking')
UPSTREAM=$(echo "$STREAM_DATA" | jq -r '.upstream[]' | sed 's/^/- /')
DOWNSTREAM=$(echo "$STREAM_DATA" | jq -r '.downstream[]' | sed 's/^/- /')

# Build test cases section
TEST_SECTION=$(echo "$STREAM_DATA" | jq -r '.tests | to_entries[] | "\(.key): \(.value)"' | sed 's/^/- /')

# Fill template
sed \
    -e "s|{ID}|$STREAM_ID|g" \
    -e "s|{NAME}|$NAME|g" \
    -e "s|{SCOPE_SUMMARY}|$SCOPE|g" \
    -e "s|{OWNED_CRATES_AND_MODULES}|$OWNED|g" \
    -e "s|{LOCKED_INTERFACE_LIST}|$LOCKED|g" \
    -e "s|{TEST_CASES}|$TEST_SECTION|g" \
    -e "s|{MOCKING_INSTRUCTIONS}|$MOCKING|g" \
    -e "s|{CRATE_NAME}|$CRATE|g" \
    -e "s|{UPSTREAM_DEPENDENCIES}|$UPSTREAM|g" \
    -e "s|{WHO_USES_YOUR_OUTPUT}|$DOWNSTREAM|g" \
    "$TEMPLATE" > "$OUTPUT_DIR/${STREAM_ID}.md"

echo "Generated: $OUTPUT_DIR/${STREAM_ID}.md"
```

---

## File 5: `streams.json`

The data file containing all 20 streams. This is the **single source of truth** — edit this once, then generate all briefs.

```json
[
  {
    "id": "S01",
    "name": "Lexer",
    "crate": "glyim-frontend",
    "wave": 1,
    "scope_summary": "Complete lexer: keywords, literals, operators, error recovery",
    "owned_crates": ["glyim-frontend::lexer"],
    "locked_interfaces": [
      "glyim_syntax::SyntaxKind (all variants, try_from_raw)",
      "glyim_span::FileId, Span, ByteIdx",
      "glyim_diag::GlyimDiagnostic (lex_error constructor)",
      "glyim_frontend::Token, LexResult (public types)"
    ],
    "tests": {
      "S01-T01": "Lex all 30+ keywords → each produces correct SyntaxKind",
      "S01-T02": "Lex decimal integers → 0, 42, 999999",
      "S01-T03": "Lex hex integers → 0xFF, 0x0",
      "S01-T04": "Lex binary integers → 0b0, 0b1010",
      "S01-T05": "Lex octal integers → 0o0, 0o77",
      "S01-T06": "Lex integer suffixes → 42u8, 0xFFi32",
      "S01-T07": "Lex float literals → 3.14, 1.0, 0.5",
      "S01-T08": "Lex float exponents → 1e10, 1.5e-3",
      "S01-T09": "Lex float error: 1e → error diagnostic, IntLit for 1",
      "S01-T10": "Lex float error: 1e+ → error diagnostic",
      "S01-T11": "Lex string literals with escapes",
      "S01-T12": "Lex char literals with escapes",
      "S01-T13": "Lex all operators → correct SyntaxKind",
      "S01-T14": "Lex compound operators → +=, -=, ::, ->, =>",
      "S01-T15": "Lex nested block comments",
      "S01-T16": "Lex unexpected character → error diagnostic with span",
      "S01-T17": "Token spans are contiguous and cover entire source",
      "S01-T18": "Empty input → empty token list, no diagnostics"
    },
    "mocking": "None needed. Pure function: &str + FileId → LexResult. Use real SyntaxKind and Span types.",
    "upstream": [],
    "downstream": ["S09-Parser"]
  },
  {
    "id": "S02",
    "name": "TypeInterning",
    "crate": "glyim-type",
    "wave": 1,
    "scope_summary": "TyCtxMut construction, mk_* methods, substitution interning, freeze",
    "owned_crates": ["glyim-type::context"],
    "locked_interfaces": [
      "glyim_type::Ty (to_raw, index, ERROR, NEVER, UNIT, BOOL)",
      "glyim_type::TyKind (all variants)",
      "glyim_type::Substitution (index, len, is_empty)",
      "glyim_type::TypeLookup trait",
      "glyim_type::TypeFlags",
      "glyim_core::interner::Interner"
    ],
    "tests": {
      "S02-T01": "Sentinel constants at correct indices",
      "S02-T02": "freeze() verifies sentinels — corrupt → panic",
      "S02-T03": "mk_ref creates Ref; roundtrip via ty_kind",
      "S02-T04": "mk_ref with Not mutability",
      "S02-T05": "mk_adt with empty Substitution",
      "S02-T06": "mk_adt with Substitution containing Ty args",
      "S02-T07": "mk_tuple with multiple tys",
      "S02-T08": "mk_fn_ptr with FnSig",
      "S02-T09": "Substitution interning deduplicates",
      "S02-T10": "Substitution with 10 args",
      "S02-T11": "error_ty() returns Ty::ERROR",
      "S02-T12": "TyCtxMut is !Send + !Sync (compile_fail test)",
      "S02-T13": "TyCtx is Send + Sync",
      "S02-T14": "Region variable allocation and retrieval"
    },
    "mocking": "None needed. Interner::new() is standalone. Use test_ty_ctx() from glyim-test.",
    "upstream": [],
    "downstream": ["S05-Unification", "S10-TypeDisplay", "S14-TypeckDriver"]
  },
  {
    "id": "S03",
    "name": "MIRCore",
    "crate": "glyim-mir",
    "wave": 1,
    "scope_summary": "Place::ty(), Body helpers, Statement/Terminator construction",
    "owned_crates": ["glyim-mir"],
    "locked_interfaces": [
      "glyim_type::Ty (ERROR, NEVER, UNIT, BOOL sentinels)",
      "glyim_type::TypeLookup trait",
      "glyim_type::TyKind (Ref, RawPtr, Slice, Array, Tuple, Adt variants)",
      "glyim_type::Substitution"
    ],
    "tests": {
      "S03-T01": "Body::dummy creates valid structure",
      "S03-T02": "Place::new creates place with empty projection",
      "S03-T03": "Place::ty() Deref on &T → returns T",
      "S03-T04": "Place::ty() Deref on &mut T → returns T",
      "S03-T05": "Place::ty() Deref on *const T → returns T",
      "S03-T06": "Place::ty() Deref on *mut T → returns T",
      "S03-T07": "Place::ty() Deref on non-pointer → Ty::ERROR + tracing::error",
      "S03-T08": "Place::ty() Field on Tuple → correct arg Ty",
      "S03-T09": "Place::ty() Field on non-tuple → Ty::ERROR",
      "S03-T10": "Place::ty() Index on [T; N] → returns T",
      "S03-T11": "Place::ty() Index on [T] → returns T",
      "S03-T12": "Place::ty() Index on non-array/slice → Ty::ERROR",
      "S03-T13": "Place::ty() Downcast returns same type",
      "S03-T14": "Place::ty() chained projections: &((i32, u32)).0 → i32"
    },
    "mocking": "Implement TypeLookup for a mock struct in tests. Return pre-built TyKind references.",
    "upstream": [],
    "downstream": ["S15-MIRLowering", "S16-Borrowck", "S17-MIROpt"]
  },
  {
    "id": "S04",
    "name": "LayoutEngine",
    "crate": "glyim-layout",
    "wave": 1,
    "scope_summary": "SimpleLayoutComputer for all primitives, reject unsized types",
    "owned_crates": ["glyim-layout"],
    "locked_interfaces": [
      "glyim_type::TyCtx",
      "glyim_type::Ty (ERROR, etc.)",
      "glyim_type::TyKind (Int, Uint, Float, Bool, Char, Ref, RawPtr, Slice, Dynamic, Error)",
      "glyim_core::primitives::TargetInfo",
      "glyim_core::abi::ALIGN_MAX"
    ],
    "tests": {
      "S04-T01": "Layout i8 → Size(1), Align(1)",
      "S04-T02": "Layout i16 → Size(2), Align(2)",
      "S04-T03": "Layout i32 → Size(4), Align(4)",
      "S04-T04": "Layout i64 → Size(8), Align(8)",
      "S04-T05": "Layout f32 → Size(4), Align(4)",
      "S04-T06": "Layout f64 → Size(8), Align(8)",
      "S04-T07": "Layout bool → Size(1), Align(1)",
      "S04-T08": "Layout unit → Size(0), Align(1)",
      "S04-T09": "Layout never → Size(0), Align(1)",
      "S04-T10": "Layout &T → pointer_size",
      "S04-T11": "Layout *const T → pointer_size",
      "S04-T12": "Layout [T] → LayoutError::Unsized",
      "S04-T13": "Layout dyn Trait → LayoutError::Unsized",
      "S04-T14": "fn_abi_of basic signature → correct ArgAbi"
    },
    "mocking": "Use test_frozen_ty_ctx() from glyim-test. Construct types with TyCtxMut then freeze.",
    "upstream": [],
    "downstream": ["S06-MIRInterpreter"]
  },
  {
    "id": "S05",
    "name": "Unification",
    "crate": "glyim-solve",
    "wave": 1,
    "scope_summary": "InferenceTable with full unification, variable resolution, kind enforcement",
    "owned_crates": ["glyim-solve::infer"],
    "locked_interfaces": [
      "glyim_type::TyCtxMut, TyCtx",
      "glyim_type::Ty, TyKind, InferVar, TyVar, IntVar, FloatVar",
      "glyim_type::Region, RegionVid",
      "glyim_type::Substitution, GenericArg",
      "glyim_type::TypeLookup",
      "glyim_type::PrintTy",
      "glyim_diag::GlyimDiagnostic (type_error)",
      "glyim_span::Span"
    ],
    "tests": {
      "S05-T01": "Unify i32 with i32 → Ok",
      "S05-T02": "Unify i32 with u32 → Err",
      "S05-T03": "Unify TyVar with i32 → binds variable",
      "S05-T04": "Unify IntVar with i32 → binds variable",
      "S05-T05": "Unify IntVar with bool → Err (expected integer)",
      "S05-T06": "Unify IntVar with f64 → Err (expected integer)",
      "S05-T07": "Unify FloatVar with f64 → binds variable",
      "S05-T08": "Unify FloatVar with i32 → Err (expected float)",
      "S05-T09": "Unify &mut T with &T → Err (mutability mismatch)",
      "S05-T10": "Unify &T with &T → recursive unify T",
      "S05-T11": "Unify error type with anything → Ok",
      "S05-T12": "resolve_ty_shallow follows one binding",
      "S05-T13": "resolve_ty_shallow follows transitive chain",
      "S05-T14": "fully_resolve returns Err for unresolved TyVar",
      "S05-T15": "fully_resolve returns Err for unresolved IntVar",
      "S05-T16": "fully_resolve returns Err for unresolved FloatVar",
      "S05-T17": "fully_resolve returns Ok for fully resolved type",
      "S05-T18": "New TyVar, IntVar, FloatVar create distinct indices"
    },
    "mocking": "Use TyCtxMut::new(Interner::new()) directly. No mocking needed.",
    "upstream": ["S02-TypeInterning"],
    "downstream": ["S14-TypeckDriver"]
  },
  {
    "id": "S06",
    "name": "MIRInterpreter",
    "crate": "glyim-mir-interp",
    "wave": 1,
    "scope_summary": "Execute arithmetic, control flow, function calls, allocations",
    "owned_crates": ["glyim-mir-interp"],
    "locked_interfaces": [
      "glyim_mir::Body, BasicBlockData, Statement, Terminator",
      "glyim_mir::TerminatorKind (Goto, Return, Unreachable, Call)",
      "glyim_mir::StatementKind (Assign, StorageLive, StorageDead, Nop)",
      "glyim_mir::Rvalue (Use, BinaryOp, Ref, Aggregate)",
      "glyim_mir::Operand, Place, LocalDecl, MirConst"
    ],
    "tests": {
      "S06-T01": "Interpret integer add → 3 + 4 = 7",
      "S06-T02": "Interpret integer sub → 10 - 3 = 7",
      "S06-T03": "Interpret branch on true → takes then_bb",
      "S06-T04": "Interpret branch on false → takes else_bb",
      "S06-T05": "Interpret function call → params passed, return value",
      "S06-T06": "Interpret infinite loop → TimedOut",
      "S06-T07": "Interpret deep recursion → StackOverflow",
      "S06-T08": "Unreachable terminator → Panic with backtrace",
      "S06-T09": "Allocate + write + read → correct value",
      "S06-T10": "Step limit default is 1_000_000",
      "S06-T11": "Recursion limit default is 256"
    },
    "mocking": "Hand-craft Body and BasicBlockData. Use Body::dummy() as starting point, then add real statements.",
    "upstream": ["S03-MIRCore"],
    "downstream": []
  },
  {
    "id": "S07",
    "name": "BytecodeBackend",
    "crate": "glyim-codegen",
    "wave": 1,
    "scope_summary": "Emit opcodes from hand-crafted MIR",
    "owned_crates": ["glyim-codegen"],
    "locked_interfaces": [
      "glyim_codegen::CodegenBackend trait",
      "glyim_mir::Body, Statement, Terminator, Rvalue, Operand"
    ],
    "tests": {
      "S07-T01": "Empty function → produces module with Return opcode",
      "S07-T02": "Function with integer constants → LoadConst + Add + Return",
      "S07-T03": "Function with locals → LoadLocal + StoreLocal",
      "S07-T04": "Branch → JumpIf + Jump opcodes",
      "S07-T05": "generate() returns non-empty Vec<u8>",
      "S07-T06": "name() returns 'bytecode'"
    },
    "mocking": "Hand-craft Body with real statement/terminator types. Use Body::dummy().",
    "upstream": ["S03-MIRCore"],
    "downstream": ["S18-Pipeline"]
  },
  {
    "id": "S08",
    "name": "LLVMBackend",
    "crate": "glyim-codegen-llvm",
    "wave": 1,
    "scope_summary": "Generate LLVM IR from hand-crafted MIR",
    "owned_crates": ["glyim-codegen-llvm"],
    "locked_interfaces": [
      "glyim_codegen::CodegenBackend trait",
      "glyim_mir::Body"
    ],
    "tests": {
      "S08-T01": "Create LlvmBackend without crash",
      "S08-T02": "generate() with empty bodies → creates module",
      "S08-T03": "generate() returns Ok",
      "S08-T04": "name() returns 'llvm'",
      "S08-T05": "Multiple generate() calls reuse Context"
    },
    "mocking": "Use Body::dummy(). Minimal MIR needed.",
    "upstream": [],
    "downstream": ["S18-Pipeline"]
  },
  {
    "id": "S09",
    "name": "Parser",
    "crate": "glyim-frontend",
    "wave": 2,
    "scope_summary": "Full recursive-descent parser for v0.1.0 grammar with error recovery",
    "owned_crates": ["glyim-frontend::parser"],
    "locked_interfaces": [
      "glyim_syntax::SyntaxKind (all variants)",
      "glyim_syntax::GlyimLang, SyntaxNode, GreenNode",
      "glyim_frontend::lexer (Token, lex function)",
      "glyim_frontend::ParseResult"
    ],
    "tests": {
      "S09-T01": "Parse fn item → FnDef node",
      "S09-T02": "Parse struct (unit, tuple, record) → StructDef",
      "S09-T03": "Parse enum with variants → EnumDef",
      "S09-T04": "Parse trait def → TraitDef",
      "S09-T05": "Parse impl def → ImplDef",
      "S09-T06": "Parse expression precedence correctly",
      "S09-T07": "Parse method calls and field access",
      "S09-T08": "Parse pattern grammar",
      "S09-T09": "Parse type grammar",
      "S09-T10": "Error recovery: missing semicolon",
      "S09-T11": "Error recovery: mismatched braces",
      "S09-T12": "No token loss: parse covers all tokens",
      "S09-T13": "Snapshot: representative program CST"
    },
    "mocking": "Uses real lexer output. If S01 incomplete, mock Token stream.",
    "upstream": ["S01-Lexer"],
    "downstream": ["S12-HIRLowering", "S13-DefMap"]
  },
  {
    "id": "S10",
    "name": "TypeDisplay",
    "crate": "glyim-type",
    "wave": 2,
    "scope_summary": "PrintTy for all TyKind variants, compute_flags complete",
    "owned_crates": ["glyim-type::display", "glyim-type::flags"],
    "locked_interfaces": [
      "glyim_type::TypeLookup trait",
      "glyim_type::Ty, TyKind (all variants)",
      "glyim_type::TypeFlags"
    ],
    "tests": {
      "S10-T01": "PrintTy renders i32, u32, f64, bool, char, !, ()",
      "S10-T02": "PrintTy renders &mut i32, &i32, *const u8",
      "S10-T03": "PrintTy renders [T], (A, B), fn(A) -> B",
      "S10-T04": "PrintTy renders Adt with substitution",
      "S10-T05": "PrintTy recursion limit → '…'",
      "S10-T06": "compute_flags detects HAS_TY_INFER",
      "S10-T07": "compute_flags detects HAS_ERROR",
      "S10-T08": "compute_flags sets HAS_DEPTH_OVERFLOW at depth > 64",
      "S10-T09": "HAS_DEPTH_OVERFLOW does NOT set HAS_ERROR",
      "S10-T10": "ty_is_error only checks HAS_ERROR",
      "S10-T11": "compute_flags propagates through Ref, Slice, Array",
      "S10-T12": "compute_flags propagates through Substitution"
    },
    "mocking": "Implement TypeLookup for mock struct. Use TyCtxMut::new() for real testing.",
    "upstream": ["S02-TypeInterning"],
    "downstream": ["S14-TypeckDriver"]
  },
  {
    "id": "S11",
    "name": "TraitSolver",
    "crate": "glyim-solve",
    "wave": 2,
    "scope_summary": "SimpleTraitSolver, TraitContext, FulfillmentCtx with BFS",
    "owned_crates": ["glyim-solve::solver", "glyim-solve::fulfill"],
    "locked_interfaces": [
      "glyim_solve::TraitSolver trait",
      "glyim_type::TyCtx",
      "glyim_type::Predicate, TraitPredicate, TraitRef"
    ],
    "tests": {
      "S11-T01": "Register trait → appears in trait_defs",
      "S11-T02": "Register impl → appears in impl_defs",
      "S11-T03": "Prove trait with matching impl → Proven",
      "S11-T04": "Prove trait with no impl → Ambiguous",
      "S11-T05": "impls_of_trait returns correct subset",
      "S11-T06": "FulfillmentCtx registers obligations",
      "S11-T07": "BFS processing order",
      "S11-T08": "Overflow protection",
      "S11-T09": "Multiple obligations all checked",
      "S11-T10": "Ambiguous → warning diagnostic",
      "S11-T11": "Definite no → error diagnostic"
    },
    "mocking": "Use TraitContext directly. No mocking needed.",
    "upstream": [],
    "downstream": ["S14-TypeckDriver"]
  },
  {
    "id": "S12",
    "name": "HIRLowering",
    "crate": "glyim-hir",
    "wave": 2,
    "scope_summary": "CST → HIR conversion producing populated CrateHir",
    "owned_crates": ["glyim-hir"],
    "locked_interfaces": [
      "glyim_syntax::SyntaxNode, SyntaxKind",
      "glyim_hir::CrateHir, Item, Expr, Pat, Body (public types)",
      "glyim_core::interner::Name"
    ],
    "tests": {
      "S12-T01": "Fn item → FnItem with params",
      "S12-T02": "Struct item (record) → StructItem",
      "S12-T03": "Struct item (unit) → StructItem",
      "S12-T04": "Enum item → EnumItem",
      "S12-T05": "Block expression → Body",
      "S12-T06": "Binary expression → Expr::Binary",
      "S12-T07": "If/else → Expr::If",
      "S12-T08": "Path expression → Expr::Path",
      "S12-T09": "Literal → Expr::Literal",
      "S12-T10": "Type references → TypeRef roundtrip",
      "S12-T11": "Pattern wild → Pat::Wild",
      "S12-T12": "Pattern binding → Pat::Binding"
    },
    "mocking": "Use parse_to_syntax() output. If S09 incomplete, construct SyntaxNodes manually.",
    "upstream": ["S09-Parser"],
    "downstream": ["S14-TypeckDriver"]
  },
  {
    "id": "S13",
    "name": "DefMap",
    "crate": "glyim-def-map",
    "wave": 3,
    "scope_summary": "Path resolution, scope population, module graph",
    "owned_crates": ["glyim-def-map"],
    "locked_interfaces": [
      "glyim_def_map::CrateDefMap, ModuleData, ItemScope",
      "glyim_def_map::build_def_map",
      "glyim_syntax::SyntaxNode",
      "glyim_core::path::Path, PathKind"
    ],
    "tests": {
      "S13-T01": "Empty file → root module",
      "S13-T02": "Single fn → appears in scope",
      "S13-T03": "Struct, enum, trait, impl → all in scope",
      "S13-T04": "Inline module → child module",
      "S13-T05": "Visibility: pub vs private",
      "S13-T06": "Path resolution: plain path",
      "S13-T07": "Path resolution: self::",
      "S13-T08": "Path resolution: super::",
      "S13-T09": "Path resolution: crate::",
      "S13-T10": "Duplicate name → error",
      "S13-T11": "Unknown name → PerNs::default()"
    },
    "mocking": "Use parse_to_syntax() output.",
    "upstream": ["S09-Parser"],
    "downstream": ["S14-TypeckDriver"]
  },
  {
    "id": "S14",
    "name": "TypeckDriver",
    "crate": "glyim-typeck",
    "wave": 3,
    "scope_summary": "typeck_crate with real inference, THIR construction",
    "owned_crates": ["glyim-typeck"],
    "locked_interfaces": [
      "glyim_typeck::typeck_crate",
      "glyim_typeck::TypeckResult",
      "glyim_type::TyCtxMut, TyCtx",
      "glyim_solve::InferenceTable, TraitSolver",
      "glyim_hir::CrateHir",
      "glyim_def_map::CrateDefMap"
    ],
    "tests": {
      "S14-T01": "Typecheck empty crate → no errors",
      "S14-T02": "Typecheck fn returning () → Ok",
      "S14-T03": "Typecheck i32 + i32 → i32",
      "S14-T04": "Typecheck i32 + bool → error",
      "S14-T05": "Typecheck &x where x: i32",
      "S14-T06": "Typecheck &mut x",
      "S14-T07": "Inference: let x = 42 → i32",
      "S14-T08": "Obligation collection",
      "S14-T09": "Obligation fulfillment",
      "S14-T10": "THIR body construction"
    },
    "mocking": "Hand-craft CrateHir and CrateDefMap using test_ty_ctx(). Build minimal HIR with known types.",
    "upstream": ["S05-Unification", "S11-TraitSolver", "S12-HIRLowering", "S13-DefMap"],
    "downstream": ["S15-MIRLowering"]
  },
  {
    "id": "S15",
    "name": "MIRLowering",
    "crate": "glyim-lower",
    "wave": 3,
    "scope_summary": "THIR → MIR producing Body with real basic blocks",
    "owned_crates": ["glyim-lower"],
    "locked_interfaces": [
      "glyim_lower::lower_body, LowerCtx",
      "glyim_typeck::thir::Body",
      "glyim_mir::Body"
    ],
    "tests": {
      "S15-T01": "Lower empty function → Return",
      "S15-T02": "Lower params → locals allocated",
      "S15-T03": "Lower let binding → StorageLive + Assign",
      "S15-T04": "Lower return → Return terminator",
      "S15-T05": "Lower binary op → BinaryOp rvalue",
      "S15-T06": "Lower if-else → SwitchInt",
      "S15-T07": "Lower function call → Call terminator",
      "S15-T08": "Lower reference → Ref rvalue"
    },
    "mocking": "Hand-craft thir::Body using Ty::ERROR sentinels and minimal types.",
    "upstream": ["S14-TypeckDriver"],
    "downstream": ["S16-Borrowck", "S17-MIROpt"]
  },
  {
    "id": "S16",
    "name": "Borrowck",
    "crate": "glyim-borrowck",
    "wave": 3,
    "scope_summary": "Extract region constraints, check borrow conflicts",
    "owned_crates": ["glyim-borrowck"],
    "locked_interfaces": [
      "glyim_borrowck::check_borrows, BorrowckCtx, BorrowckResult",
      "glyim_mir::Body"
    ],
    "tests": {
      "S16-T01": "No borrows → no errors",
      "S16-T02": "Two shared borrows → no error",
      "S16-T03": "Shared + mutable → error",
      "S16-T04": "Two mutable → error",
      "S16-T05": "Borrow expires after last use",
      "S16-T06": "Region constraints extracted",
      "S16-T07": "Error diagnostics include span",
      "S16-T08": "BorrowckCtx.local_ty returns correct type"
    },
    "mocking": "Hand-craft Body with known borrow patterns.",
    "upstream": ["S03-MIRCore"],
    "downstream": ["S18-Pipeline"]
  },
  {
    "id": "S17",
    "name": "MIROpt",
    "crate": "glyim-opt",
    "wave": 3,
    "scope_summary": "Constant propagation, DCE, CFG simplification",
    "owned_crates": ["glyim-opt"],
    "locked_interfaces": [
      "glyim_opt::optimize, Optimized",
      "glyim_type::TyCtx",
      "glyim_mir::Body"
    ],
    "tests": {
      "S17-T01": "Constant propagation",
      "S17-T02": "Dead code elimination",
      "S17-T03": "CFG simplification",
      "S17-T04": "Unreachable block elimination",
      "S17-T05": "No-op pass"
    },
    "mocking": "Hand-craft Body with optimization opportunities.",
    "upstream": ["S03-MIRCore"],
    "downstream": ["S18-Pipeline"]
  },
  {
    "id": "S18",
    "name": "Pipeline",
    "crate": "glyim-pipeline",
    "wave": 4,
    "scope_summary": "End-to-end compile_file driving all 9 phases",
    "owned_crates": ["glyim-pipeline", "glyim-db"],
    "locked_interfaces": [
      "glyim_pipeline::Pipeline::compile_file",
      "glyim_db::Database",
      "glyim_codegen::CodegenBackend"
    ],
    "tests": {
      "S18-T01": "Compile empty file → Ok",
      "S18-T02": "Compile fn main() {} → Ok",
      "S18-T03": "Compile type error → diagnostic",
      "S18-T04": "Compile syntax error → diagnostic",
      "S18-T05": "Missing file → I/O error",
      "S18-T06": "Backend selection"
    },
    "mocking": "Real files on disk. Real Database.",
    "upstream": ["ALL"],
    "downstream": []
  },
  {
    "id": "S19",
    "name": "LSP",
    "crate": "glyim-lsp",
    "wave": 4,
    "scope_summary": "did_open, did_change, diagnostics, URI handling",
    "owned_crates": ["glyim-lsp"],
    "locked_interfaces": [
      "glyim_db::Database",
      "glyim_diag::GlyimDiagnostic"
    ],
    "tests": {
      "S19-T01": "did_open registers file",
      "S19-T02": "did_open with content → file_content returns same",
      "S19-T03": "URI: Unix roundtrip",
      "S19-T04": "URI: Windows roundtrip",
      "S19-T05": "URI: no scheme",
      "S19-T06": "path_to_uri roundtrip",
      "S19-T07": "Byte offset to position"
    },
    "mocking": "Database instance.",
    "upstream": ["S18-Pipeline"],
    "downstream": []
  },
  {
    "id": "S20",
    "name": "CLI",
    "crate": "glyim-cli",
    "wave": 4,
    "scope_summary": "Argument parsing, backend selection, miette rendering",
    "owned_crates": ["glyim-cli"],
    "locked_interfaces": [
      "glyim_cli::CliArgs, run",
      "glyim_pipeline::Pipeline",
      "glyim_codegen::CodegenBackend"
    ],
    "tests": {
      "S20-T01": "Compile valid file → exit 0",
      "S20-T02": "Compile invalid file → exit 1",
      "S20-T03": "--help",
      "S20-T04": "Missing input → error",
      "S20-T05": "--backend bytecode"
    },
    "mocking": "None.",
    "upstream": ["S18-Pipeline"],
    "downstream": []
  }
]
```

---

## How to Use This System

### Step 1: Create the 5 files

```bash
mkdir -p agent-kit/briefs
# Create all 5 files above in agent-kit/
```

### Step 2: Generate all 20 stream briefs

```bash
chmod +x agent-kit/generate-stream.sh

for id in S01 S02 S03 S04 S05 S06 S07 S08 S09 S10 S11 S12 S13 S14 S15 S16 S17 S18 S19 S20; do
    ./agent-kit/generate-stream.sh "$id"
done
```

### Step 3: Dispatch an agent

Give each agent exactly these documents:

| Document | Purpose | How |
|----------|---------|-----|
| `AGENT_MASTER_CONTEXT.md` | Project-wide rules | Include in system prompt |
| `CONTRACTS_LOCKED.md` | What they can't touch | Include in system prompt |
| `briefs/S{XX}.md` | Their specific mission + tests | Include in user prompt |
| Their owned crate source files | What they work on | Attach as context |
| `glyim-test/src/lib.rs` | Test helpers | Attach if relevant |
| Relevant `glyim-type/src/*.rs` | Type system contracts | Attach for mocking |

### Step 4: The actual prompt template

```
You are implementing Stream S{ID}: {NAME} for the Glyim compiler.

## Master Context
{PASTE: AGENT_MASTER_CONTEXT.md}

## Locked Contracts
{PASTE: CONTRACTS_LOCKED.md}

## Your Stream Brief
{PASTE: briefs/S{ID}.md}

## Source Code Context
{PASTE: relevant crate source files}

## Instructions
1. Read your stream brief completely
2. Write ALL test cases first — verify they compile
3. Implement until all tests pass
4. Run verification commands from your brief
5. Output: complete modified files with full content, no abbreviations
```

This system means you write the 5 files **once**, then for each stream you just run the generator and paste 4 documents into the agent prompt. Maximum parallelism, maximum consistency, minimum manual work.
