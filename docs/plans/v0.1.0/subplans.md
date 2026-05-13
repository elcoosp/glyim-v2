# Glyim v0.1.0 — Parallel Subplan Decomposition

## Execution Wave Model

```
Wave 0 ──► Subplan 0 (Foundation) ─────────────────────────────────────────────┐
                                                                                 │
Wave 1 ──► Subplan A ──► Subplan B ──► Subplan C ──► Subplan D ──► Subplan E ──►│
           (Frontend)    (Meta)        (Type Core)   (Analysis)     (MIR/Lower) │
                                                                                 │
Wave 1 ──► Subplan F (Safety/Opt) ───────────── depends on C, E ────────────────┤
                                                                                 │
Wave 2 ──► Subplan G (Backend/Integration) ───── depends on ALL ────────────────┘
```

**Concurrency Rules:**
- Subplan 0 **must** complete before any other subplan starts.
- Subplans A, B, C can start **immediately** after Subplan 0.
- Subplan D can start once A and C have published their contracts (stubs compiling).
- Subplan E can start once C and D have published their contracts.
- Subplan F can start once C and E have published their contracts.
- Subplan G starts after all others are green.

---

## Global Rules (Inherited by Every Subplan)

These rules are **binding** on every subplan. Copy them into each dispatch.

1. **API-first with context.** Every crate publishes its final public interface before implementation begins.
2. **`TyCtx` is the spine.** Post-typeck, `TyCtx` is frozen, `Send + Sync`, passed as `&TyCtx`. No `&mut TyCtx` beyond typeck.
3. **Real interning.** All Chalk associated types are `u32` indices into arenas. `Substitution` is interned. No `Box`/`Vec` pretending to intern.
4. **Tracing mandated.** Every public function in a logic crate carries `#[tracing::instrument(skip(...))]`.
5. **Stubs allowed, mock-driven.** Implementations may begin as `todo!()`. Each crate supports a `mock` feature.
6. **Contract governance.** Changes to public types require a written "Contract Change Request" reviewed by at least one other track lead.
7. **Minimal v0.1.0.** Generators, virtual dispatch shims, coverage, retag, and niche optimizations are excluded.
8. **No `unsafe` without a safety proof.** Every `unsafe` block has a `// SAFETY:` comment.
9. **Primitives isolation.** `glyim-primitives` is the only shared enum crate. Frontend crates never depend on `glyim-type`.
10. **All workspace `Cargo.toml` entries and dependency versions are fixed** as specified in the workspace manifest. Do not add or upgrade dependencies without a Contract Change Request.

### Findings That Must Be Upheld

| ID | Rule for Implementers |
|----|----------------------|
| C1 | `TyCtx` is pure `Vec`/`IndexVec`. No `RwLock` or any lock in frozen context. `ChalkArenas` dropped on `freeze()`. |
| C2 | `GlyimChalkInterner` is `Copy` holding `*mut ChalkArenas`, scoped to `&mut TyCtxMut`. No globals, no thread-locals. |
| C3 | `BorrowckCtx::local_ty` default returns `self.local_decl(local).ty` directly. |
| C4 | No manual `unsafe impl Send/Sync` for `TyCtx`. It auto-derives because all fields are `Send + Sync`. |
| B1 | `HygieneKey` uses `unsafe fn new_unchecked`. Only `glyim-hygiene` may call it. |
| B2 | `glyim-hir` depends on `glyim-primitives` only, never on `glyim-type`. |
| M1 | Chalk interner state is local to `TyCtxMut`, not global. |
| M2 | `TyCtxMut` uses no `RwLock`; mutation via `&mut self` only. |
| P1 | `intern_substitution` uses `IndexSet<Vec<GenericArg>>` with `insert_full`; zero heap alloc on hit. |
| D1 | No thread-local `*const TyCtx` for debug. Use `PrintTy(ty, ctx)` explicitly. |
| E1 | Raw pointer for Chalk interner is scoped to `TyCtxMut`. |
| E2 | `process_obligations(limit)` returns `Err(OverflowError)`. |
| E3 | `unsafe fn new_unchecked` is the standard pattern for invariant encapsulation, not feature gates. |

---

# Subplan 0 — Foundation

## Metadata

| Field | Value |
|-------|-------|
| **Subplan ID** | S0 |
| **Prerequisites** | None |
| **Crates** | `glyim-arena`, `glyim-primitives`, `glyim-interner`, `glyim-span`, `glyim-diag` |
| **Estimated Duration** | 2–3 days |
| **Exit Criterion** | All 5 crates compile, `cargo test` green, `cargo check --all-targets` clean |

## Dependency Graph Within Subplan

```
glyim-arena ──────────────────────────────┐
glyim-primitives ─────────────────────────┤  (no cross-deps, fully parallel)
glyim-interner ───────────────────────────┤
glyim-span ───────────────────────────────┤
    └──► glyim-diag                      ┘
```

## Crate Contracts

### Crate 1: `glyim-arena`

**Directory:** `crates/glyim-arena/`

**Cargo.toml:**
```toml
[package]
name = "glyim-arena"
edition.workspace = true
version.workspace = true

[dependencies]
```

**Public API (`src/lib.rs`):**

Implement the following exactly as specified. No additions, no omissions.

- **`Idx<T>`** — Zero-cost newtype over `u32` with `PhantomData<T>`. Derives `Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord`. `Debug` prints `Idx({raw})`.
  - `from_raw(raw: u32) -> Self`
  - `index(self) -> usize`
- **`IdxLike` trait** — `Copy + Eq + Debug + 'static`. Methods: `from_raw(u32) -> Self`, `to_raw(self) -> u32`, `index(self) -> usize` (default via `to_raw`).
- **`define_idx!($name:ident)` macro** — Emits newtype `struct $name(pub u32)` with `from_raw`, `index`, and `IdxLike` impl. Uses `$crate::IdxLike` for correct cross-crate resolution.
- **`IndexVec<I: IdxLike, T>`** — `raw: Vec<T>` + `PhantomData<I>`. Derives `Clone, Debug`.
  - Constructors: `new()`, `with_capacity(usize)`, `from_raw(Vec<T>)`
  - Mutation: `push(val: T) -> I`, `reserve(usize)`
  - Access: `len()`, `is_empty()`, `try_index(I) -> Option<&T>`, `try_index_mut(I) -> Option<&mut T>`, `get(I)`, `get_mut(I)`
  - Iteration: `iter()`, `iter_mut()`, `iter_enumerated() -> (I, &T)`, `into_iter_enumerated() -> (I, T)`
  - Conversion: `into_raw()`, `as_slice()`, `as_mut_slice()`, `last()`
  - `Default` impl returns `new()`
  - `Index<I>` and `IndexMut<I>` — panic with descriptive message including `type_name::<I>()` on OOB.
- **`empty_iter<T>()`** — Returns `std::iter::empty()`.

**Testing Requirements:**
- Test `IndexVec` push/index round-trip.
- Test OOB panics (with `#[should_panic]`).
- Test `define_idx!` generates a usable `IdxLike` impl.
- Test `iter_enumerated` yields correct indices.

---

### Crate 2: `glyim-primitives`

**Directory:** `crates/glyim-primitives/`

**Cargo.toml:**
```toml
[package]
name = "glyim-primitives"
edition.workspace = true
version.workspace = true

[dependencies]
```

**Public API (`src/lib.rs`):**

Implement the following enums and impls exactly:

- **`IntTy`**: `I8 | I16 | I32 | I64 | Isize` — `bit_width() -> u32`, `name() -> &'static str`
- **`UintTy`**: `U8 | U16 | U32 | U64 | Usize` — `bit_width() -> u32`, `name() -> &'static str`
- **`FloatTy`**: `F32 | F64` — `bit_width() -> u32`, `name() -> &'static str`
- **`Mutability`**: `Not | Mut` — `is_mut() -> bool`
- **`Safety`**: `Safe | Unsafe`
- **`Abi`**: `C | Glyim | System` — `name() -> &'static str`
- **`BinOp`**: `Add | Sub | Mul | Div | Rem | Eq | Ne | Lt | Gt | LtEq | GtEq | And | Or | BitAnd | BitOr | BitXor | Shl | Shr` — `is_comparison()`, `is_lazy()`, `is_arithmetic()`, `is_bitwise()`
- **`UnOp`**: `Not | Neg | Deref`
- **`Visibility`**: `Public | Module(u32) | Inherited`
- **`StructKind`**: `Unit | Tuple | Record`

All enums derive `Clone, Copy, Debug, PartialEq, Eq, Hash`.

**Testing Requirements:**
- Verify `bit_width` and `name` for all numeric types.
- Verify `BinOp` classification methods.

---

### Crate 3: `glyim-interner`

**Directory:** `crates/glyim-interner/`

**Cargo.toml:**
```toml
[package]
name = "glyim-interner"
edition.workspace = true
version.workspace = true

[dependencies]
lasso = { workspace = true }
```

**Public API (`src/lib.rs`):**

- **`Symbol`** = `lasso::Spur` (re-export)
- **`Name`** = `Symbol` (type alias)
- **`Interner`** — wraps `lasso::ThreadedRodeo`.
  - `new() -> Self`
  - `intern(&self, s: &str) -> Name`
  - `resolver(&self) -> Resolver`
- **`Resolver`** — `Clone`, wraps `ThreadedRodeo`.
  - `resolve(&self, name: Name) -> &str` (panics on unknown)
  - `lookup(&self, s: &str) -> Option<Name>`
  - `Debug` impl: non-exhaustive struct display
- **`Kw`** struct — All fields listed: `fn_`, `let_`, `struct_`, `enum_`, `if_`, `else_`, `return_`, `match_`, `mod_`, `comptime`, `self_`, `super_`, `crate_`, `true_`, `false_`, `mut_`, `ref_`, `as_`, `while_`, `for_`, `in_`, `break_`, `continue_`, `trait_`, `impl_`, `where_`, `type_`, `pub_`, `priv_`, `extern_`, `unsafe_`, `const_`, `static_`, `underscore`, `self_type`, `bool`, `i8`–`f64`, `char`, `string`, `never`, `unit`.
  - `new(interner: &Interner) -> Self`
- **`InternedString`** — Pairs `Name` + `Resolver`.
  - `new(name: Name, resolver: Resolver) -> Self`
  - `as_str(&self) -> &str`
  - `Debug` prints quoted string. `Display` prints unquoted.

**Testing Requirements:**
- Intern then resolve round-trip.
- `Kw::new` populates all fields without panic.
- `InternedString` Display/Debug formatting.

---

### Crate 4: `glyim-span`

**Directory:** `crates/glyim-span/`

**Cargo.toml:**
```toml
[package]
name = "glyim-span"
edition.workspace = true
version.workspace = true

[dependencies]
miette = { workspace = true }
```

**Public API (`src/lib.rs`):**

- **`FileId(u32)`** — `BOGUS = FileId(u32::MAX)`
- **`ByteIdx(u32)`** — `ZERO = ByteIdx(0)`, `to_usize() -> usize`
- **`Span`** — 16 bytes, `Copy`. Fields: `file: FileId`, `lo: ByteIdx`, `hi: ByteIdx`, `ctx: SyntaxContext`.
  - `DUMMY` constant
  - `new(file, lo, hi, ctx)` — debug_assert `lo <= hi`
  - `is_dummy()`, `range() -> Range<usize>`, `sans_ctx()`, `to(other)`, `contains(other)`, `len() -> u32`
  - `From<Span> for SourceSpan`
- **`SyntaxContext(u32)`** — `ROOT = SyntaxContext(0)`. `is_root()`, `to_raw()`, `from_raw(HygieneKey, u32)`
- **`ExpnId(u32)`** — `ROOT = ExpnId(0)`. `is_root()`, `to_raw()`, `from_raw(HygieneKey, u32)`
- **`Transparency`** — `Transparent | SemiTransparent | Opaque`
- **`HygieneKey(())`** — ZST. `unsafe fn new_unchecked() -> Self` with SAFETY doc: "Must only be called by the `glyim-hygiene` crate."
- **`MultiSpan`** — `primary: Span`, `secondary: Vec<(Span, String)>`. `from_span()`, `with_secondary()`. `From<Span>`.

**Testing Requirements:**
- `Span::new` debug_assert fires on lo > hi (test with `#[should_panic]`).
- `Span::to` merges correctly.
- `Span::contains` works.
- `SyntaxContext::ROOT.is_root()` is true.
- `HygieneKey::new_unchecked` compiles in unsafe context.

---

### Crate 5: `glyim-diag`

**Directory:** `crates/glyim-diag/`

**Cargo.toml:**
```toml
[package]
name = "glyim-diag"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-span = { workspace = true }
miette = { workspace = true }
```

**Public API (`src/lib.rs`):**

- Re-exports: `miette::{Diagnostic, IntoDiagnostic, Report, Severity, SourceSpan}`, `glyim_span::{Span, MultiSpan}`
- **`ErrorCode`** — `category: ErrorCategory`, `number: u16`. `Display` as `"{cat}{:04}"`.
- **`ErrorCategory`** — `Lex | Parse | NameResolution | Type | Lifetime | Borrow | Comptime | Io | Internal` with prefixes E/P/N/T/L/B/C/I/X.
- **`DiagSeverity`** — `Error | Warning | Note | Help`
- **`SubDiagnostic`** — `severity`, `message: String`, `span: Option<MultiSpan>`
- **`Suggestion`** — `message`, `replacements: Vec<(Span, String)>`, `applicability: Applicability`
- **`Applicability`** — `MachineApplicable | MaybeIncorrect | HasPlaceholders | Unspecified`
- **`GlyimDiagnostic`** — `code`, `severity`, `message`, `span`, `sub_diagnostics`, `suggestions`.
  - `new()`, convenience constructors: `lex_error`, `parse_error`, `name_error`, `type_error`, `lifetime_error`, `borrow_error`, `comptime_error`, `internal_error`
  - Builder methods: `with_sub()`, `with_suggestion()`, `is_error()`
- **`CompResult<T>`** = `Result<T, Vec<GlyimDiagnostic>>`
- **`DiagSink`** — `diagnostics: Vec`, `error_count`, `warning_count`, `error_limit` (default 50), `limited`.
  - `new()`, `with_error_limit()`, `emit()`, `has_errors()`, `error_count()`, `warning_count()`, `was_limited()`, `into_diagnostics()`, `diagnostics()`
  - `Default` impl. `Extend<GlyimDiagnostic>` impl.

**Testing Requirements:**
- `ErrorCode` Display formatting.
- `DiagSink` error limit: emit 51 errors, verify only 50 + "too many errors" message.
- Convenience constructors produce correct `ErrorCategory`.

---

# Subplan A — Frontend

## Metadata

| Field | Value |
|-------|-------|
| **Subplan ID** | SA |
| **Prerequisites** | Subplan 0 complete |
| **Crates** | `glyim-vfs`, `glyim-syntax`, `glyim-lex`, `glyim-parse`, `glyim-def-map` |
| **Estimated Duration** | 6–8 weeks |
| **Exit Criterion** | All 5 crates compile. `mock` feature works. Parser passes snapshot tests for valid programs. `def-map` resolves names for simple multi-file projects. |

## Dependency Graph Within Subplan

```
glyim-vfs ──────────────────────┐
glyim-syntax ◄── glyim-primitives│
    │                           │
    ├──► glyim-lex ◄── glyim-interner
    │         │
    │         └──► glyim-parse
    │                   │
    └───────────────────┴──► glyim-def-map
```

## Crate Contracts

### Crate 1: `glyim-vfs`

**Directory:** `crates/glyim-vfs/`

**Cargo.toml:**
```toml
[package]
name = "glyim-vfs"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-span = { workspace = true }
parking_lot = { workspace = true }
```

**Public API — implement as specified in the plan document:**

- **`Vfs`** — Thread-safe via `RwLock`. Fields: `files: RwLock<Vec<VfsFile>>`, `path_to_id: RwLock<HashMap<PathBuf, FileId>>`, `next_id: RwLock<u32>`.
- `new()`, `add_file(&self, path: &Path) -> FileId`, `add_file_content(&self, path: &Path, content: String) -> FileId`, `set_file_content(&self, file_id: FileId, content: String)`, `file_content(&self, file_id: FileId) -> Option<String>`, `file_content_ref(&self, file_id: FileId, f: impl FnOnce(&str) -> R) -> Option<R>`, `file_path(&self, file_id: FileId) -> Option<PathBuf>`, `file_id(&self, path: &Path) -> Option<FileId>`, `len()`, `is_empty()`.
- All mutating methods carry `#[tracing::instrument(skip(self))]` or `skip(self, content)`.
- `Default` impl.

**Testing:**
- Add file, retrieve by `FileId`, retrieve by path.
- `set_file_content` updates in place.
- Idempotent `add_file` returns same `FileId`.
- `file_content_ref` avoids clone.

---

### Crate 2: `glyim-syntax`

**Directory:** `crates/glyim-syntax/`

**Cargo.toml:**
```toml
[package]
name = "glyim-syntax"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-primitives = { workspace = true }
rowan = { workspace = true }
```

**Public API:**

- **`SyntaxKind`** — `repr(u16)` enum with ALL variants listed in the plan (keywords, literals, operators, punctuation, delimiters, trivia, nodes, Error).
  - `is_trivia()`, `is_keyword()`, `is_literal()`, `is_node()`, `is_token()`, `try_from_raw(u16) -> Option<Self>`
- **`GlyimLang`** — Implements `rowan::Language` with `Kind = SyntaxKind`.
- Type aliases: `SyntaxNode`, `SyntaxToken`, `SyntaxElement`, `SyntaxNodeChildren`, `SyntaxElementChildren`, `GreenNode`, `GreenToken`, `GreenElement`, `Cursor`.
- **`AstNode` trait** — `can_cast(kind)`, `cast(node)`, `syntax(&self)`.
- Helpers: `child_of_kind()`, `token_of_kind()`.

**Testing:**
- `SyntaxKind::try_from_raw` round-trips.
- `GlyimLang` kind conversion round-trips.
- Construct a `GreenNode` and verify CST traversal.

---

### Crate 3: `glyim-lex`

**Directory:** `crates/glyim-lex/`

**Cargo.toml:**
```toml
[package]
name = "glyim-lex"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-syntax = { workspace = true }
glyim-interner = { workspace = true }
glyim-span = { workspace = true }
glyim-diag = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Lexer: source text → token stream.

/// Lexical error emitted into `DiagSink`.
pub struct LexError { /* private */ }

/// A single token with its kind, span, and optional resolved name.
#[derive(Clone, Debug)]
pub struct Token {
    pub kind: SyntaxKind,
    pub span: Span,
    pub name: Option<Name>,  // resolved for identifiers/keywords
}

/// Tokenize source text.
///
/// Returns tokens and emits diagnostics into `sink`.
/// The final token is always `SyntaxKind::Eof`.
#[tracing::instrument(skip(source, interner, sink))]
pub fn tokenize(
    source: &str,
    file_id: FileId,
    interner: &Interner,
    sink: &mut DiagSink,
) -> Vec<Token>;

/// Re-lex a single token at a given position (for LSP semantic tokens).
#[tracing::instrument(skip(source))]
pub fn relex_token_at(
    source: &str,
    file_id: FileId,
    byte_offset: usize,
) -> Option<Token>;
```

**Testing:**
- Snapshot test: tokenize all keywords, operators, literals.
- Error recovery: unterminated string, unclosed block comment.
- `mock` feature: return hand-crafted token streams.

---

### Crate 4: `glyim-parse`

**Directory:** `crates/glyim-parse/`

**Cargo.toml:**
```toml
[package]
name = "glyim-parse"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-syntax = { workspace = true }
glyim-lex = { workspace = true }
glyim-interner = { workspace = true }
glyim-span = { workspace = true }
glyim-diag = { workspace = true }
rowan = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Parser: token stream → CST (Concrete Syntax Tree).

/// Parse result containing the green tree and diagnostics.
pub struct Parse {
    green: GreenNode,
    errors: Vec<GlyimDiagnostic>,
}

impl Parse {
    /// The root syntax node of the CST.
    pub fn syntax(&self) -> SyntaxNode;

    /// All errors encountered during parsing.
    pub fn errors(&self) -> &[GlyimDiagnostic];

    /// Whether parsing completed without errors.
    pub fn ok(&self) -> bool;
}

/// Parse source text into a CST.
#[tracing::instrument(skip(source, interner, sink))]
pub fn parse_source(
    source: &str,
    file_id: FileId,
    interner: &Interner,
    sink: &mut DiagSink,
) -> Parse;

/// Parse just an expression (for REPL / LSP completion).
#[tracing::instrument(skip(source, interner))]
pub fn parse_expr_only(
    source: &str,
    file_id: FileId,
    interner: &Interner,
) -> Parse;
```

**Testing:**
- Snapshot tests using `insta` for: function definitions, struct/enum, if/match/while/for expressions, operator precedence, error recovery (missing semicolons, mismatched braces).
- `mock` feature: return hand-crafted `Parse` objects.

---

### Crate 5: `glyim-def-map`

**Directory:** `crates/glyim-def-map/`

**Cargo.toml:**
```toml
[package]
name = "glyim-def-map"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-syntax = { workspace = true }
glyim-interner = { workspace = true }
glyim-span = { workspace = true }
glyim-diag = { workspace = true }
indexmap = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Name resolution: CST → DefMap (per-module definition tables).

/// A local definition ID within a module.
define_idx!(LocalDefId);

/// A module ID in the definition map.
define_idx!(ModuleId);

/// Kind of a resolved definition.
#[derive(Clone, Debug)]
pub enum DefKind {
    Fn,
    Struct,
    Enum,
    Variant,
    Const,
    Static,
    TypeAlias,
    Trait,
    Impl,
    Module,
}

/// A resolved definition.
#[derive(Clone, Debug)]
pub struct Def {
    pub kind: DefKind,
    pub name: Name,
    pub span: Span,
    pub visibility: Visibility,
    pub module: ModuleId,
}

/// Per-module definition table.
#[derive(Clone, Debug)]
pub struct ModuleData {
    pub module_id: ModuleId,
    pub parent: Option<ModuleId>,
    pub children: IndexMap<Name, ModuleId>,
    pub definitions: IndexVec<LocalDefId, Def>,
    pub scope: IndexMap<Name, (DefKind, LocalDefId)>,
}

/// The complete definition map for a compilation unit.
#[derive(Clone, Debug)]
pub struct DefMap {
    pub modules: IndexVec<ModuleId, ModuleData>,
    pub root: ModuleId,
}

/// Build the definition map from a CST root.
#[tracing::instrument(skip(source_file, interner, sink))]
pub fn build_def_map(
    source_file: &SyntaxNode,
    file_id: FileId,
    interner: &Interner,
    sink: &mut DiagSink,
) -> DefMap;
```

**Testing:**
- Single file: resolve all top-level items.
- Multi-module: resolve `mod foo;` child modules.
- Visibility: private items not visible outside module.
- `mock` feature: return hand-crafted `DefMap`.

---

# Subplan B — Metaprogramming

## Metadata

| Field | Value |
|-------|-------|
| **Subplan ID** | SB |
| **Prerequisites** | Subplan 0 complete; Subplan A `glyim-syntax` contract published |
| **Crates** | `glyim-hygiene`, `glyim-mir-interp`, `glyim-meta` |
| **Estimated Duration** | 6–8 weeks |
| **Exit Criterion** | All 3 crates compile. Hygiene applies correct `SyntaxContext`. `mir-interp` evaluates constant expressions. `glyim-meta` expands a trivial macro. |

## Dependency Graph Within Subplan

```
glyim-hygiene ◄── glyim-span
                    │
glyim-mir-interp ◄── glyim-mir (stub contract from SE)
    │
    └──► glyim-meta ◄── glyim-hygiene, glyim-syntax
```

## Crate Contracts

### Crate 1: `glyim-hygiene`

**Directory:** `crates/glyim-hygiene/`

**Cargo.toml:**
```toml
[package]
name = "glyim-hygiene"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-span = { workspace = true }
```

**Contract to Publish:**

```rust
//! Hygiene context management.
//!
//! This is the ONLY crate that may call `HygieneKey::new_unchecked()`.
//! It manages `SyntaxContext` and `ExpnId` allocation.

/// Global hygiene state (not thread-local — passed explicitly).
pub struct HygieneCtx {
    // private
}

impl HygieneCtx {
    pub fn new() -> Self;

    /// Apply a macro expansion to a syntax context.
    /// Returns a new `SyntaxContext` with the expansion recorded.
    pub fn apply_expn(
        &mut self,
        ctx: SyntaxContext,
        expn_id: ExpnId,
        transparency: Transparency,
    ) -> SyntaxContext;

    /// Create a new expansion ID for a macro invocation.
    pub fn new_expn_id(&mut self, span: Span, name: Name) -> ExpnId;

    /// Mark a span as coming from a macro expansion.
    pub fn mark_span(&mut self, span: Span, expn_id: ExpnId, transparency: Transparency) -> Span;

    /// Adjust a syntax context for resolve (removes transparent layers).
    pub fn adjust(&self, ctx: SyntaxContext) -> SyntaxContext;

    /// Check if two identifiers with different contexts should be equated.
    pub fn identifies_same(&self, name: Name, ctx1: SyntaxContext, ctx2: SyntaxContext) -> bool;
}
```

**Key Invariant:** `HygieneCtx` internally calls `unsafe { HygieneKey::new_unchecked() }` and passes it to `SyntaxContext::from_raw` / `ExpnId::from_raw`. No other crate may do this.

**Testing:**
- `apply_expn` produces non-root contexts.
- `adjust` strips transparent layers.
- `identifies_same` respects transparency rules.
- Span marking preserves file/offset, changes only `ctx`.

---

### Crate 2: `glyim-mir-interp`

**Directory:** `crates/glyim-mir-interp/`

**Cargo.toml:**
```toml
[package]
name = "glyim-mir-interp"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-mir = { workspace = true }
glyim-type = { workspace = true }
glyim-diag = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Constant evaluation via MIR interpretation.

use glyim_mir::Body;
use glyim_type::TyCtx;

/// A computed constant value.
#[derive(Clone, Debug)]
pub enum ConstValue {
    Int(i128),
    Uint(u128),
    Float(u64),  // bits
    Bool(bool),
    Char(char),
    Unit,
}

/// Interpret a MIR body to compute a constant value.
///
/// Returns `Err` if the body cannot be evaluated at compile time
/// (e.g., contains side effects, function calls, loops beyond limit).
#[tracing::instrument(skip(ctx, body))]
pub fn interpret_const(
    ctx: &TyCtx,
    body: &Body,
    fuel: usize,
) -> Result<ConstValue, InterpError>;

/// Error during interpretation.
#[derive(Clone, Debug)]
pub enum InterpError {
    Unsupported(String),
    Overflow,
    OutOfFuel,
    Panic(String),
}
```

**Testing:**
- Interpret simple arithmetic: `2 + 3 * 4` → `14`.
- Overflow detection.
- Fuel limit enforcement.
- `mock` feature: return hand-crafted `ConstValue`.

**Note:** Depends on `glyim-mir` and `glyim-type` stubs. Use the contracts published by Subplan C/SE. During Phase 2, these are `todo!()` stubs.

---

### Crate 3: `glyim-meta`

**Directory:** `crates/glyim-meta/`

**Cargo.toml:**
```toml
[package]
name = "glyim-meta"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-syntax = { workspace = true }
glyim-span = { workspace = true }
glyim-hygiene = { workspace = true }
glyim-diag = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Macro expansion engine.

/// Result of expanding a macro invocation.
#[derive(Clone, Debug)]
pub struct Expansion {
    /// The CST nodes produced by the expansion.
    pub nodes: Vec<SyntaxNode>,
    /// The expansion ID for hygiene tracking.
    pub expn_id: ExpnId,
}

/// Expand a single macro call.
#[tracing::instrument(skip(hygiene_ctx, sink))]
pub fn expand_macro(
    call: &SyntaxNode,  // MacroCall node
    hygiene_ctx: &mut HygieneCtx,
    sink: &mut DiagSink,
) -> Result<Expansion, GlyimDiagnostic>;
```

**Testing:**
- Expand a `comptime` block that evaluates a constant expression.
- Hygiene: identifiers in macro output have correct `SyntaxContext`.
- `mock` feature: return hand-crafted `Expansion`.

---

# Subplan C — Type System Core

## Metadata

| Field | Value |
|-------|-------|
| **Subplan ID** | SC |
| **Prerequisites** | Subplan 0 complete |
| **Crates** | `glyim-type`, `glyim-infer`, `glyim-solve`, `glyim-layout` |
| **Estimated Duration** | 6–8 weeks |
| **Exit Criterion** | All 4 crates compile. `TyCtx` freeze works. Type inference resolves simple programs. Trait solving returns correct obligations. Layout computes sizes for primitives and structs. |

## Dependency Graph Within Subplan

```
glyim-type ◄── glyim-arena, glyim-primitives, chalk-ir, chalk-solve, indexmap
    │
    ├──► glyim-infer
    │
    ├──► glyim-solve
    │
    └──► glyim-layout
```

## Crate Contracts

### Crate 1: `glyim-type`

**Directory:** `crates/glyim-type/`

**Cargo.toml:**
```toml
[package]
name = "glyim-type"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-arena = { workspace = true }
glyim-primitives = { workspace = true }
glyim-span = { workspace = true }
chalk-ir = { workspace = true }
chalk-solve = { workspace = true }
indexmap = { workspace = true }
smallvec = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Core type system: types, type context, interner.
//!
//! CRITICAL INVARIANTS:
//! - C1: TyCtx is pure Vec/IndexVec after freeze(). No locks.
//! - C2: GlyimChalkInterner is Copy, holds *mut ChalkArenas, scoped to &mut TyCtxMut.
//! - C4: TyCtx auto-derives Send + Sync (no unsafe impl).
//! - M1: No global state for Chalk interner.
//! - M2: TyCtxMut uses no RwLock; mutation via &mut self only.
//! - P1: intern_substitution uses IndexSet, zero heap alloc on hit.
//! - D1: No thread-local *const TyCtx. Use PrintTy(ty, ctx) for display.

use chalk_ir::{self, TyKind, Scalar, UintTy as ChalkUintTy, IntTy as ChalkIntTy, FloatTy as ChalkFloatTy};
use glyim_arena::{Idx, IdxLike, IndexVec, define_idx};
use glyim_primitives::{IntTy, UintTy, FloatTy, Mutability, Safety, Abi};
use glyim_span::Span;
use indexmap::IndexSet;

// ── Index types ──

define_idx!(TyIdx);       // Index into TyCtx.types
define_idx!(LifetimeIdx); // Index into TyCtx.lifetimes
define_idx!(ConstIdx);    // Index into TyCtx.consts
define_idx!(RegionIdx);   // Index into TyCtx.regions

// ── Glyim Ty ──

/// A type index. Thin wrapper around u32 for type safety.
/// Actual type data lives in TyCtx.types[ty.idx].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ty {
    pub idx: TyIdx,
}

impl Ty {
    pub fn from_idx(idx: TyIdx) -> Self { Self { idx } }
}

// ── Type data ──

#[derive(Clone, Debug)]
pub enum TyData {
    Int(IntTy),
    Uint(UintTy),
    Float(FloatTy),
    Bool,
    Char,
    Never,
    Str,
    Ref(Mutability, Ty, RegionIdx),
    RawPtr(Mutability, Ty),
    Slice(Ty),
    Array(Ty, ConstIdx),
    Tuple(Vec<Ty>),
    Adt(AdtIdx, Substitution),
    FnDef(FnDefIdx, Substitution),
    FnPtr(FnSig),
    Closure(ClosureIdx, Substitution),
    Opaque(OpaqueTyIdx, Substitution),
    Param(TypeParamIdx),
    Error,
}

// ── Substitution ──

/// Interned substitution. [P1] Stored in IndexSet for zero-alloc lookup.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Substitution {
    pub indices: Vec<GenericArg>,
}

// ── GenericArg ──

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GenericArg {
    Ty(Ty),
    Lifetime(RegionIdx),
    Const(ConstIdx),
}

// ── FnSig ──

#[derive(Clone, Debug)]
pub struct FnSig {
    pub inputs: Vec<Ty>,
    pub output: Ty,
    pub safety: Safety,
    pub abi: Abi,
    pub c_variadic: bool,
}

// ── Definition indices ──

define_idx!(AdtIdx);
define_idx!(FnDefIdx);
define_idx!(ClosureIdx);
define_idx!(OpaqueTyIdx);
define_idx!(TypeParamIdx);

// ── ChalkArenas: the mutable backing store ──

/// All Chalk interned data. Dropped on freeze().
/// [C1] [M2] No RwLock. Only accessible via &mut TyCtxMut.
pub struct ChalkArenas {
    types: IndexSet<TyData>,
    substitutions: IndexSet<Vec<GenericArg>>,  // [P1]
    fn_sigs: IndexSet<FnSig>,
}

// ── GlyimChalkInterner ──

/// [C2] Copy + holds *mut ChalkArenas. Scoped to &mut TyCtxMut.
/// No globals, no thread-locals.
#[derive(Clone, Copy)]
pub struct GlyimChalkInterner {
    arenas: *mut ChalkArenas,
}

impl GlyimChalkInterner {
    /// Create from a mutable reference. The pointer is valid
    /// only for the lifetime of the TyCtxMut borrow.
    pub fn new(arenas: &mut ChalkArenas) -> Self {
        Self { arenas: arenas as *mut ChalkArenas }
    }
}

// SAFETY: The pointer is only used during &mut TyCtxMut scope.
// TyCtxMut is !Sync, so no concurrent access.
unsafe impl Send for GlyimChalkInterner {}
unsafe impl Sync for GlyimChalkInterner {}

// ── TyCtxMut: the mutable type context ──

/// Mutable type context for type inference and construction.
/// [M2] No RwLock. All mutation via &mut self.
pub struct TyCtxMut {
    arenas: ChalkArenas,
    types: IndexVec<TyIdx, TyData>,
    lifetimes: IndexVec<LifetimeIdx, LifetimeData>,
    consts: IndexVec<ConstIdx, ConstData>,
    regions: IndexVec<RegionIdx, RegionData>,
    adt_defs: IndexVec<AdtIdx, AdtData>,
    fn_defs: IndexVec<FnDefIdx, FnDefData>,
    closures: IndexVec<ClosureIdx, ClosureData>,
    opaque_tys: IndexVec<OpaqueTyIdx, OpaqueTyData>,
    type_params: IndexVec<TypeParamIdx, TypeParamData>,
}

impl TyCtxMut {
    pub fn new() -> Self;

    /// Get the Chalk interner, scoped to this mutable context.
    pub fn interner(&mut self) -> GlyimChalkInterner;

    /// Intern a type, returning its index.
    pub fn intern_ty(&mut self, data: TyData) -> Ty;

    /// Intern a substitution. [P1] Zero heap alloc on cache hit.
    pub fn intern_substitution(&mut self, sub: Substitution) -> Substitution;

    /// Freeze the context, consuming mutation capability.
    /// [C1] Drops ChalkArenas. Returns frozen TyCtx.
    pub fn freeze(self) -> TyCtx;
}

// ── TyCtx: the frozen type context ──

/// Frozen type context. [C1] No locks. [C4] Auto-derived Send + Sync.
/// Passed as &TyCtx after typeck.
pub struct TyCtx {
    types: IndexVec<TyIdx, TyData>,
    lifetimes: IndexVec<LifetimeIdx, LifetimeData>,
    consts: IndexVec<ConstIdx, ConstData>,
    regions: IndexVec<RegionIdx, RegionData>,
    adt_defs: IndexVec<AdtIdx, AdtData>,
    fn_defs: IndexVec<FnDefIdx, FnDefData>,
    closures: IndexVec<ClosureIdx, ClosureData>,
    opaque_tys: IndexVec<OpaqueTyIdx, OpaqueTyData>,
    type_params: IndexVec<TypeParamIdx, TypeParamData>,
}

impl TyCtx {
    pub fn ty_data(&self, ty: Ty) -> &TyData;
    pub fn fn_sig(&self, fn_def: FnDefIdx) -> &FnSig;
    pub fn adt_data(&self, adt: AdtIdx) -> &AdtData;
    // ... read-only accessors for all tables
}

// Note: TyCtx contains only IndexVec and Vec, which are Send + Sync.
// No unsafe impl needed. [C4]

// ── Pretty printing ──

/// [D1] Explicit pretty-printer. No thread-local TyCtx.
pub struct PrintTy<'a>(pub Ty, pub &'a TyCtx);

impl<'a> std::fmt::Display for PrintTy<'a> { ... }
```

**Additional types to define (stubs OK in Phase 2):**
- `LifetimeData`, `ConstData`, `RegionData`, `AdtData`, `FnDefData`, `ClosureData`, `OpaqueTyData`, `TypeParamData`
- Chalk `Interner` impl for `GlyimChalkInterner` — all associated types are `u32` indices per [Rule 3].

**Testing:**
- `TyCtxMut::new()` → `intern_ty` → `freeze()` → `TyCtx::ty_data` round-trip.
- `intern_substitution` returns same index for identical substitution (dedup).
- `TyCtx: Send + Sync` compiles.
- `PrintTy` displays types correctly.

---

### Crate 2: `glyim-infer`

**Directory:** `crates/glyim-infer/`

**Cargo.toml:**
```toml
[package]
name = "glyim-infer"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-type = { workspace = true }
glyim-arena = { workspace = true }
chalk-ir = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Type inference engine.

define_idx!(InferVarIdx);

/// An unresolved type inference variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InferVar { pub idx: InferVarIdx }

/// Inference context. Borrows TyCtxMut.
pub struct InferenceCtx<'a> {
    ctx: &'a mut TyCtxMut,
    var_table: IndexVec<InferVarIdx, InferVarKind>,
    obligations: Vec<Obligation>,
}

pub enum InferVarKind {
    Ty(InferTyVar),
    Lifetime,
    Const(Ty),
}

pub struct InferTyVar {
    pub origin: InferVarOrigin,
    pub bound: Option<Ty>,
}

pub enum InferVarOrigin {
    TypeAnnotation(Span),
    Expr(Span),
    Param(Span),
}

/// An unsatisfied obligation.
#[derive(Clone, Debug)]
pub struct Obligation {
    pub predicate: Predicate,
    pub cause: ObligationCause,
    pub depth: usize,
}

#[derive(Clone, Debug)]
pub struct ObligationCause {
    pub span: Span,
    pub code: ObligationCauseCode,
}

#[derive(Clone, Debug)]
pub enum ObligationCauseCode {
    WellFormed,
    ExprAssign,
    MethodCall,
    ReturnExpr,
}

pub enum Predicate {
    TraitImplemented(Ty, TraitRef),
    TypeWellFormed(Ty),
    TypeHasLocalImpl(Ty),
}

#[derive(Clone, Debug)]
pub struct TraitRef {
    pub trait_id: TraitIdx,
    pub self_ty: Ty,
    pub sub: Substitution,
}

impl<'a> InferenceCtx<'a> {
    pub fn new(ctx: &'a mut TyCtxMut) -> Self;

    /// Create a fresh type inference variable.
    pub fn fresh_ty_var(&mut self, origin: InferVarOrigin) -> InferVar;

    /// Create a fresh integer variable (for integer inference).
    pub fn fresh_int_var(&mut self, origin: InferVarOrigin) -> InferVar;

    /// Create a fresh float variable.
    pub fn fresh_float_var(&mut self, origin: InferVarOrigin) -> InferVar;

    /// Unify two types.
    pub fn unify(&mut self, expected: Ty, actual: Ty, span: Span) -> Result<(), TypeError>;

    /// Record an obligation.
    pub fn register_obligation(&mut self, obligation: Obligation);

    /// Get all pending obligations.
    pub fn pending_obligations(&self) -> &[Obligation];
}

#[derive(Clone, Debug)]
pub struct TypeError {
    pub expected: Ty,
    pub actual: Ty,
    pub span: Span,
}
```

**Testing:**
- Unify two concrete types succeeds.
- Unify `InferVar` with concrete type resolves correctly.
- Occurs-check: unifying `T = Vec<T>` fails.
- `mock` feature.

---

### Crate 3: `glyim-solve`

**Directory:** `crates/glyim-solve/`

**Cargo.toml:**
```toml
[package]
name = "glyim-solve"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-type = { workspace = true }
glyim-arena = { workspace = true }
chalk-solve = { workspace = true }
chalk-ir = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Trait solving via Chalk.

/// Result of solving a trait obligation.
#[derive(Clone, Debug)]
pub enum Solution {
    /// Unique solution (definite yes).
    Unique(Substitution),
    /// Ambiguous — multiple solutions possible.
    Ambiguous,
    /// No solution.
    None,
}

/// Process a batch of obligations with overflow protection.
/// [E2] Returns Err(OverflowError) if limit exceeded.
#[tracing::instrument(skip(ctx, obligations))]
pub fn process_obligations(
    ctx: &mut TyCtxMut,
    obligations: &[Obligation],
    limit: usize,
) -> Result<Vec<Solution>, OverflowError>;

/// Solve a single predicate.
#[tracing::instrument(skip(ctx))]
pub fn solve_predicate(
    ctx: &mut TyCtxMut,
    predicate: &Predicate,
) -> Solution;

#[derive(Clone, Debug)]
pub struct OverflowError;
```

**Testing:**
- Solve `impl Trait for Type` → `Unique`.
- Solve with no impl → `None`.
- Overlapping impls → `Ambiguous`.
- Depth limit → `Err(OverflowError)`.
- `mock` feature.

---

### Crate 4: `glyim-layout`

**Directory:** `crates/glyim-layout/`

**Cargo.toml:**
```toml
[package]
name = "glyim-layout"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-type = { workspace = true }
glyim-arena = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Memory layout computation.

/// Computed layout for a type.
#[derive(Clone, Debug)]
pub struct Layout {
    pub size: u64,
    pub align: u64,
    pub stride: u64,
    pub kind: LayoutKind,
}

#[derive(Clone, Debug)]
pub enum LayoutKind {
    Scalar,
    Struct { field_offsets: Vec<u64> },
    Enum { discriminant_offset: u64, variants: Vec<Layout> },
    Array { count: u64, element_layout: Box<Layout> },
    Union,
}

/// Compute the layout of a type from a frozen TyCtx.
#[tracing::instrument(skip(ctx))]
pub fn layout_of(ctx: &TyCtx, ty: Ty) -> Option<Layout>;

/// Compute the ABI size/alignment of a function signature.
#[tracing::instrument(skip(ctx))]
pub fn fn_abi_of(ctx: &TyCtx, sig: &FnSig) -> Option<FnAbi>;

#[derive(Clone, Debug)]
pub struct FnAbi {
    pub ret_layout: Option<Layout>,
    pub arg_layouts: Vec<Layout>,
    pub conv: CallingConvention,
}

#[derive(Clone, Copy, Debug)]
pub enum CallingConvention {
    C,
    Glyim,
    System,
}
```

**Testing:**
- Layout of primitives: `i32` → size=4, align=4.
- Layout of structs with alignment padding.
- Layout of enums with discriminant.
- Layout of nested types.
- `mock` feature.

---

# Subplan D — Analysis & HIR/THIR

## Metadata

| Field | Value |
|-------|-------|
| **Subplan ID** | SD |
| **Prerequisites** | Subplan 0; Subplan A `glyim-syntax` contract; Subplan C `glyim-type` contract |
| **Crates** | `glyim-hir`, `glyim-thir`, `glyim-typeck` |
| **Estimated Duration** | 6–8 weeks |
| **Exit Criterion** | HIR/THIR lowered from parse. Typeck resolves types for simple programs. Consumes `TyCtxMut`, produces `TyCtx`. |

## Dependency Graph Within Subplan

```
glyim-hir ◄── glyim-primitives, glyim-syntax (NOT glyim-type!) [B2]
    │
    └──► glyim-thir ◄── glyim-hir, glyim-type
              │
              └──► glyim-typeck ◄── glyim-type, glyim-infer, glyim-solve
```

## Crate Contracts

### Crate 1: `glyim-hir`

**Directory:** `crates/glyim-hir/`

**Cargo.toml:**
```toml
[package]
name = "glyim-hir"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-primitives = { workspace = true }  # [B2] NOT glyim-type
glyim-syntax = { workspace = true }
glyim-arena = { workspace = true }
glyim-span = { workspace = true }
glyim-interner = { workspace = true }
smallvec = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! High-level Intermediate Representation.
//!
//! [B2] This crate does NOT depend on glyim-type.
//! It uses only glyim-primitives for shared enums.

define_idx!(HirBodyIdx);
define_idx!(HirItemIdx);
define_idx!(HirExprIdx);
define_idx!(HirPatIdx);
define_idx!(HirStmtIdx);

/// A top-level HIR item.
#[derive(Clone, Debug)]
pub enum HirItem {
    Fn(HirFn),
    Struct(HirStruct),
    Enum(HirEnum),
    Const(HirConst),
    Static(HirStatic),
    TypeAlias(HirTypeAlias),
    Trait(HirTrait),
    Impl(HirImpl),
    Module(HirModule),
}

/// HIR function.
#[derive(Clone, Debug)]
pub struct HirFn {
    pub name: Name,
    pub span: Span,
    pub params: Vec<HirParam>,
    pub return_ty: HirTy,
    pub body: Option<HirBodyIdx>,
    pub generics: HirGenerics,
    pub safety: Safety,
    pub abi: Abi,
}

/// HIR type representation (no Chalk, just syntactic).
#[derive(Clone, Debug)]
pub enum HirTy {
    Path(HirPath),
    Ref(Mutability, Box<HirTy>),
    Slice(Box<HirTy>),
    Array(Box<HirTy>, HirExprIdx),
    Tuple(Vec<HirTy>),
    FnPtr(HirFnSig),
    Infer,
    Never,
    Error,
}

/// HIR expression.
#[derive(Clone, Debug)]
pub enum HirExpr {
    Lit(HirLit),
    Path(HirPath),
    Ref(Mutability, HirExprIdx),
    Unary(UnOp, HirExprIdx),
    Binary(BinOp, HirExprIdx, HirExprIdx),
    Call { func: HirExprIdx, args: Vec<HirExprIdx> },
    MethodCall { receiver: HirExprIdx, method: Name, args: Vec<HirExprIdx> },
    Field { expr: HirExprIdx, name: Name },
    Index { base: HirExprIdx, index: HirExprIdx },
    If { cond: HirExprIdx, then: HirBodyIdx, else_: Option<HirBodyIdx> },
    Match { scrutinee: HirExprIdx, arms: Vec<HirMatchArm> },
    While { cond: HirExprIdx, body: HirBodyIdx },
    For { pat: HirPatIdx, iter: HirExprIdx, body: HirBodyIdx },
    Break(HirExprIdx),
    Continue,
    Return(HirExprIdx),
    Assign { target: HirExprIdx, value: HirExprIdx },
    Cast { expr: HirExprIdx, ty: HirTy },
    Closure { params: Vec<HirParam>, body: HirBodyIdx },
    Error,
}

/// HIR body (block of statements with optional tail expression).
#[derive(Clone, Debug)]
pub struct HirBody {
    pub stmts: Vec<HirStmtIdx>,
    pub tail: Option<HirExprIdx>,
}

/// The complete HIR for a compilation unit.
#[derive(Clone, Debug)]
pub struct Hir {
    pub items: IndexVec<HirItemIdx, HirItem>,
    pub bodies: IndexVec<HirBodyIdx, HirBody>,
    pub exprs: IndexVec<HirExprIdx, HirExpr>,
    pub pats: IndexVec<HirPatIdx, HirPat>,
    pub stmts: IndexVec<HirStmtIdx, HirStmt>,
}

/// Lower CST to HIR.
#[tracing::instrument(skip(source_file, interner, sink))]
pub fn lower_cst_to_hir(
    source_file: &SyntaxNode,
    interner: &Interner,
    sink: &mut DiagSink,
) -> Hir;
```

**Testing:**
- Lower simple fn def, struct, enum.
- Lower expressions: if, match, binary ops.
- Verify HIR does not import `glyim_type`.
- `mock` feature.

---

### Crate 2: `glyim-thir`

**Directory:** `crates/glyim-thir/`

**Cargo.toml:**
```toml
[package]
name = "glyim-thir"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-hir = { workspace = true }
glyim-type = { workspace = true }
glyim-arena = { workspace = true }
glyim-span = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Typed High-level Intermediate Representation.
//!
//! THIR is HIR where every expression carries a resolved type.

define_idx!(ThirExprIdx);
define_idx!(ThirPatIdx);
define_idx!(ThirStmtIdx);

/// THIR expression with resolved types.
#[derive(Clone, Debug)]
pub struct ThirExpr {
    pub kind: ThirExprKind,
    pub ty: Ty,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ThirExprKind {
    Lit(ThirLit),
    Var(LocalVarIdx),
    Ref(Mutability, ThirExprIdx),
    Unary(UnOp, ThirExprIdx),
    Binary(BinOp, ThirExprIdx, ThirExprIdx),
    Call { func: ThirExprIdx, args: Vec<ThirExprIdx>, fn_sig: FnSig },
    MethodCall { receiver: ThirExprIdx, method: Name, args: Vec<ThirExprIdx> },
    Field { expr: ThirExprIdx, name: Name, field_idx: FieldIdx },
    Index { base: ThirExprIdx, index: ThirExprIdx },
    If { cond: ThirExprIdx, then: ThirBodyIdx, else_: Option<ThirBodyIdx> },
    Match { scrutinee: ThirExprIdx, arms: Vec<ThirMatchArm> },
    // ... mirrors HirExpr but with types
    Error,
}

/// Lower HIR to THIR using type information.
#[tracing::instrument(skip(ctx, hir, sink))]
pub fn lower_hir_to_thir(
    ctx: &TyCtx,
    hir: &Hir,
    sink: &mut DiagSink,
) -> Thir;
```

**Testing:**
- Lower typed HIR items.
- Every `ThirExpr` has a non-error `Ty` for well-typed programs.
- `mock` feature.

---

### Crate 3: `glyim-typeck`

**Directory:** `crates/glyim-typeck/`

**Cargo.toml:**
```toml
[package]
name = "glyim-typeck"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-hir = { workspace = true }
glyim-type = { workspace = true }
glyim-infer = { workspace = true }
glyim-solve = { workspace = true }
glyim-thir = { workspace = true }
glyim-diag = { workspace = true }
glyim-span = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Type checking: HIR + TyCtxMut → TyCtx + THIR.
//!
//! This is the entry point for type checking. It consumes TyCtxMut
//! and produces a frozen TyCtx.

/// Type checking context.
pub struct TypeckCtx<'a> {
    ctx: &'a mut TyCtxMut,
    infer: InferenceCtx<'a>,
}

/// Result of type checking.
pub struct TypeckResult {
    pub ty_ctx: TyCtx,
    pub thir: Thir,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

/// Type check all items in the HIR.
/// Consumes TyCtxMut, produces frozen TyCtx.
#[tracing::instrument(skip(ctx_mut, hir))]
pub fn typecheck(
    ctx_mut: TyCtxMut,
    hir: &Hir,
) -> TypeckResult;

/// Type check a single function body.
#[tracing::instrument(skip(ctx, hir, body_idx))]
pub fn typecheck_fn(
    ctx: &mut TypeckCtx<'_>,
    hir: &Hir,
    body_idx: HirBodyIdx,
) -> Result<(), Vec<GlyimDiagnostic>>;

/// [E2] Fulfill obligations with overflow protection.
#[tracing::instrument(skip(ctx))]
pub fn fulfill_obligations(
    ctx: &mut TypeckCtx<'_>,
    limit: usize,
) -> Result<(), OverflowError>;
```

**Testing:**
- Typecheck simple fn: `fn add(a: i32, b: i32) -> i32 { a + b }`.
- Type error: mismatched return type produces `GlyimDiagnostic`.
- Obligation fulfillment with overflow limit.
- `TyCtx` is frozen after `typecheck()`.
- `mock` feature.

---

# Subplan E — MIR & Lowering

## Metadata

| Field | Value |
|-------|-------|
| **Subplan ID** | SE |
| **Prerequisites** | Subplan 0; Subplan C `glyim-type` contract; Subplan D `glyim-thir` contract |
| **Crates** | `glyim-mir`, `glyim-lower`, `glyim-mono` |
| **Estimated Duration** | 6–8 weeks |
| **Exit Criterion** | MIR structs compile. THIR→MIR lowering works for basic functions. Monomorphization produces specialized instances. |

## Dependency Graph Within Subplan

```
glyim-mir ◄── glyim-type, glyim-arena
    │
    ├──► glyim-lower ◄── glyim-thir, glyim-mir
    │
    └──► glyim-mono ◄── glyim-mir, glyim-type
```

## Crate Contracts

### Crate 1: `glyim-mir`

**Directory:** `crates/glyim-mir/`

**Cargo.toml:**
```toml
[package]
name = "glyim-mir"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-type = { workspace = true }
glyim-arena = { workspace = true }
glyim-span = { workspace = true }
smallvec = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Mid-level Intermediate Representation.

define_idx!(BasicBlockIdx);
define_idx!(LocalIdx);
define_idx!(PlaceIdx);
define_idx!(RvalueIdx);
define_idx!(StatementIdx);
define_idx!(TerminatorIdx);

/// A MIR body.
#[derive(Clone, Debug)]
pub struct Body {
    pub blocks: IndexVec<BasicBlockIdx, BasicBlockData>,
    pub locals: IndexVec<LocalIdx, LocalDecl>,
    pub arg_count: usize,
    pub return_ty: Ty,
    pub span: Span,
}

/// A basic block.
#[derive(Clone, Debug)]
pub struct BasicBlockData {
    pub statements: Vec<Statement>,
    pub terminator: Option<Terminator>,
}

/// Local variable declaration.
#[derive(Clone, Debug)]
pub struct LocalDecl {
    pub ty: Ty,
    pub span: Span,
    pub mutability: Mutability,
    pub name: Option<Name>,
}

/// A place (lvalue).
#[derive(Clone, Debug)]
pub struct Place {
    pub local: LocalIdx,
    pub projections: Vec<PlaceProjection>,
}

#[derive(Clone, Debug)]
pub enum PlaceProjection {
    Deref,
    Field(FieldIdx),
    Index(LocalIdx),
    Downcast(VariantIdx),
}

/// An rvalue (computed value).
#[derive(Clone, Debug)]
pub enum Rvalue {
    Use(Operand),
    Ref(Mutability, Place),
    UnaryOp(UnOp, Operand),
    BinaryOp(BinOp, Operand, Operand),
    Cast(Operand, Ty),
    Aggregate(AggregateKind, Vec<Operand>),
    Discriminant(Place),
    Len(Place),
}

#[derive(Clone, Debug)]
pub enum Operand {
    Copy(Place),
    Move(Place),
    Const(ConstIdx),
}

#[derive(Clone, Debug)]
pub enum AggregateKind {
    Array(Ty),
    Tuple,
    Adt(AdtIdx, VariantIdx, Substitution),
    Closure(ClosureIdx),
}

/// A statement.
#[derive(Clone, Debug)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum StatementKind {
    Assign(Place, Rvalue),
    SetDiscriminant(Place, VariantIdx),
    StorageLive(LocalIdx),
    StorageDead(LocalIdx),
    Deinit(Place),
    Nop,
}

/// A terminator.
#[derive(Clone, Debug)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TerminatorKind {
    Goto(BasicBlockIdx),
    SwitchInt { discr: Operand, targets: SwitchTargets },
    Return,
    Unreachable,
    Call { func: Operand, args: Vec<Operand>, destination: Place, target: Option<BasicBlockIdx> },
    Assert { cond: Operand, expected: bool, target: BasicBlockIdx, msg: AssertMessage },
    Drop { place: Place, target: BasicBlockIdx },
}

#[derive(Clone, Debug)]
pub struct SwitchTargets {
    pub values: Vec<u128>,
    pub targets: Vec<BasicBlockIdx>,
    pub otherwise: BasicBlockIdx,
}

#[derive(Clone, Debug)]
pub enum AssertMessage {
    Overflow(BinOp),
    DivisionByZero,
    RemainderByZero,
    BoundsCheck,
}
```

**Testing:**
- Construct a trivial `Body` with one block, one return.
- Round-trip through serialize/deserialize (if implemented).
- `mock` feature.

---

### Crate 2: `glyim-lower`

**Directory:** `crates/glyim-lower/`

**Cargo.toml:**
```toml
[package]
name = "glyim-lower"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-thir = { workspace = true }
glyim-mir = { workspace = true }
glyim-type = { workspace = true }
glyim-arena = { workspace = true }
glyim-diag = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Lowering: THIR → MIR.

/// Lowering context. Takes &TyCtx for type queries.
pub struct LowerCtx<'a> {
    ctx: &'a TyCtx,
}

impl<'a> LowerCtx<'a> {
    pub fn new(ctx: &'a TyCtx) -> Self;
}

/// Lower a THIR body to MIR.
#[tracing::instrument(skip(lower_ctx, thir, body_idx))]
pub fn lower_body(
    lower_ctx: &LowerCtx<'_>,
    thir: &Thir,
    body_idx: ThirBodyIdx,
) -> Body;

/// Lower all THIR bodies to MIR.
#[tracing::instrument(skip(lower_ctx, thir))]
pub fn lower_all(lower_ctx: &LowerCtx<'_>, thir: &Thir) -> Vec<(Name, Body)>;
```

**Testing:**
- Lower `fn foo() -> i32 { 42 }` → MIR with one block, `Assign` then `Return`.
- Lower if-expression → `SwitchInt` terminator.
- Lower loop → back-edge in CFG.
- `mock` feature.

---

### Crate 3: `glyim-mono`

**Directory:** `crates/glyim-mono/`

**Cargo.toml:**
```toml
[package]
name = "glyim-mono"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-mir = { workspace = true }
glyim-type = { workspace = true }
glyim-arena = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Monomorphization: generic MIR → specialized MIR.

define_idx!(MonoInstIdx);

/// A monomorphized instance.
#[derive(Clone, Debug)]
pub struct MonoInstance {
    pub name: Name,
    pub body: Body,
    pub sub: Substitution,
    pub is_entry_point: bool,
}

/// Monomorphization context.
pub struct MonoCtx<'a> {
    ctx: &'a TyCtx,
}

/// Collect and monomorphize all reachable instances starting from entry points.
#[tracing::instrument(skip(mono_ctx, bodies))]
pub fn monomorphize(
    mono_ctx: &MonoCtx<'_>,
    bodies: &[(Name, Body)],
    entry_points: &[Name],
) -> IndexVec<MonoInstIdx, MonoInstance>;
```

**Testing:**
- Monomorphize `fn id<T>(x: T) -> T` with `T = i32` → specialized body.
- Entry point collection.
- No infinite recursion for recursive generics.
- `mock` feature.

---

# Subplan F — Safety & Optimization

## Metadata

| Field | Value |
|-------|-------|
| **Subplan ID** | SF |
| **Prerequisites** | Subplan 0; Subplan C `glyim-type` contract; Subplan E `glyim-mir` contract |
| **Crates** | `glyim-lifetime`, `glyim-borrowck`, `glyim-opt` |
| **Estimated Duration** | 6–8 weeks |
| **Exit Criterion** | Lifetime inference resolves for simple functions. Borrow checker catches use-after-move. Optimizer reduces constant propagation. |

## Dependency Graph Within Subplan

```
glyim-lifetime ◄── glyim-mir, glyim-type
    │
    └──► glyim-borrowck ◄── glyim-mir, glyim-type, glyim-lifetime
              │
              └──► glyim-opt ◄── glyim-mir, glyim-type
```

## Crate Contracts

### Crate 1: `glyim-lifetime`

**Directory:** `crates/glyim-lifetime/`

**Cargo.toml:**
```toml
[package]
name = "glyim-lifetime"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-mir = { workspace = true }
glyim-type = { workspace = true }
glyim-arena = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Lifetime inference and region resolution.

/// Computed lifetime information for a MIR body.
#[derive(Clone, Debug)]
pub struct LifetimeInfo {
    pub region_map: IndexVec<RegionIdx, RegionInfo>,
    pub outlives: Vec<(RegionIdx, RegionIdx)>,
}

#[derive(Clone, Debug)]
pub struct RegionInfo {
    pub kind: RegionKind,
}

#[derive(Clone, Debug)]
pub enum RegionKind {
    EarlyBound(Name),
    LateBound(Name),
    Infer,
    Static,
}

/// Compute lifetime constraints and resolve regions.
#[tracing::instrument(skip(ctx, body))]
pub fn compute_lifetimes(
    ctx: &TyCtx,
    body: &Body,
) -> LifetimeInfo;
```

**Testing:**
- Single borrow: `let x; let r = &x;` → region outlives.
- Nested borrows.
- `mock` feature.

---

### Crate 2: `glyim-borrowck`

**Directory:** `crates/glyim-borrow/`

**Cargo.toml:**
```toml
[package]
name = "glyim-borrowck"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-mir = { workspace = true }
glyim-type = { workspace = true }
glyim-lifetime = { workspace = true }
glyim-arena = { workspace = true }
glyim-diag = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Borrow checking: verifies ownership and borrowing rules.

/// Context provided to the borrow checker.
/// [C3] Default `local_ty` returns `self.local_decl(local).ty` directly.
pub trait BorrowckCtx {
    fn local_decl(&self, local: LocalIdx) -> &LocalDecl;

    fn local_ty(&self, local: LocalIdx) -> Ty {
        self.local_decl(local).ty
    }
}

/// Result of borrow checking.
#[derive(Clone, Debug)]
pub struct BorrowckResult {
    pub move_errors: Vec<MoveError>,
    pub borrow_errors: Vec<BorrowError>,
    pub use_after_move: Vec<UseAfterMoveError>,
}

#[derive(Clone, Debug)]
pub struct MoveError {
    pub span: Span,
    pub moved_at: Span,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct BorrowError {
    pub span: Span,
    pub kind: BorrowErrorKind,
}

#[derive(Clone, Debug)]
pub enum BorrowErrorKind {
    MutBorrowOfMutRef,
    AliasedMutBorrow,
    BorrowOfMoved,
}

#[derive(Clone, Debug)]
pub struct UseAfterMoveError {
    pub span: Span,
    pub moved_at: Span,
}

/// Run borrow checking on a MIR body.
#[tracing::instrument(skip(ctx, body, sink))]
pub fn borrowck(
    ctx: &dyn BorrowckCtx,
    body: &Body,
    lifetime_info: &LifetimeInfo,
    sink: &mut DiagSink,
) -> BorrowckResult;
```

**Testing:**
- Valid program passes borrowck.
- Use after move detected.
- Aliased mutable borrow detected.
- [C3] Default `local_ty` works correctly.
- `mock` feature.

---

### Crate 3: `glyim-opt`

**Directory:** `crates/glyim-opt/`

**Cargo.toml:**
```toml
[package]
name = "glyim-opt"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-mir = { workspace = true }
glyim-type = { workspace = true }
glyim-arena = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! MIR optimization passes.

/// An optimization pass.
pub trait MirPass {
    fn name(&self) -> &'static str;
    fn run(&self, ctx: &TyCtx, body: &mut Body);
}

/// Constant propagation and folding.
pub struct ConstProp;

/// Dead code elimination.
pub struct DeadCodeElim;

/// Simplify control flow (remove unreachable blocks).
pub struct SimplifyCfg;

/// Copy propagation.
pub struct CopyPropagation;

/// Run a sequence of optimization passes.
#[tracing::instrument(skip(ctx, body, passes))]
pub fn run_optimization_pipeline(
    ctx: &TyCtx,
    body: &mut Body,
    passes: &[&dyn MirPass],
);

/// Default optimization pipeline for v0.1.0.
pub fn default_pipeline() -> Vec<Box<dyn MirPass>>;
```

**Testing:**
- `ConstProp`: `let x = 2 + 3;` → `let x = 5;`.
- `DeadCodeElim`: removes unused assignments.
- `SimplifyCfg`: removes unreachable blocks.
- `mock` feature.

---

# Subplan G — Backends & Integration

## Metadata

| Field | Value |
|-------|-------|
| **Subplan ID** | SG |
| **Prerequisites** | ALL prior subplans complete |
| **Crates** | `glyim-codegen`, `glyim-codegen-llvm`, `glyim-bytecode`, `glyim-mir-interp`, `glyim-db`, `glyim-cli`, `glyim-lsp`, `glyim-runtime`, `glyim-test` |
| **Estimated Duration** | 6–8 weeks + 2 weeks integration + 2–3 weeks polish |
| **Exit Criterion** | First `main() → LLVM IR` pipeline. CLI compiles and runs a program. LSP responds to hover/goto-def. |

## Dependency Graph Within Subplan

```
glyim-codegen ◄── glyim-mir, glyim-type, glyim-layout
    │
    ├──► glyim-codegen-llvm ◄── glyim-codegen, inkwell
    │
    ├──► glyim-bytecode ◄── glyim-codegen
    │
glyim-runtime
    │
glyim-db ◄── all prior crates (owns TyCtx)
    │
    ├──► glyim-cli ◄── glyim-db, glyim-codegen-llvm
    │
    ├──► glyim-lsp ◄── glyim-db, tokio, async-lsp, tower
    │
    └──► glyim-test ◄── glyim-db, insta
```

## Crate Contracts

### Crate 1: `glyim-codegen`

**Directory:** `crates/glyim-codegen/`

**Cargo.toml:**
```toml
[package]
name = "glyim-codegen"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-mir = { workspace = true }
glyim-type = { workspace = true }
glyim-layout = { workspace = true }
glyim-arena = { workspace = true }

[features]
mock = []
```

**Contract to Publish:**

```rust
//! Code generation abstraction layer.

/// A compiled module (target-independent representation).
#[derive(Clone, Debug)]
pub struct CompiledModule {
    pub functions: Vec<CompiledFunction>,
    pub statics: Vec<CompiledStatic>,
}

#[derive(Clone, Debug)]
pub struct CompiledFunction {
    pub name: Name,
    pub linkage: Linkage,
    pub visibility: Visibility,
    pub abi: CallingConvention,
}

#[derive(Clone, Debug)]
pub struct CompiledStatic { pub name: Name }

#[derive(Clone, Copy, Debug)]
pub enum Linkage { External, Internal }

/// Code generation trait. Implemented by backends.
pub trait CodegenBackend {
    fn compile_module(&self, ctx: &TyCtx, instances: &[MonoInstance]) -> CompiledModule;
}
```

---

### Crate 2: `glyim-codegen-llvm`

**Directory:** `crates/glyim-codegen-llvm/`

**Cargo.toml:**
```toml
[package]
name = "glyim-codegen-llvm"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-codegen = { workspace = true }
glyim-mir = { workspace = true }
glyim-type = { workspace = true }
glyim-layout = { workspace = true }
inkwell = { workspace = true }

[features]
mock = []
```

**Contract:**

```rust
//! LLVM backend via Inkwell.

pub struct LlvmBackend {
    context: inkwell::context::Context,
    module: inkwell::module::Module,
}

impl CodegenBackend for LlvmBackend {
    fn compile_module(&self, ctx: &TyCtx, instances: &[MonoInstance]) -> CompiledModule;
}

impl LlvmBackend {
    pub fn new(module_name: &str) -> Self;
    pub fn emit_llvm_ir(&self) -> String;
    pub fn emit_obj(&self, path: &Path) -> Result<(), std::io::Error>;
}
```

---

### Crate 3: `glyim-bytecode`

**Directory:** `crates/glyim-bytecode/`

**Cargo.toml:**
```toml
[package]
name = "glyim-bytecode"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-codegen = { workspace = true }
glyim-mir = { workspace = true }
glyim-type = { workspace = true }

[features]
mock = []
```

**Contract:**

```rust
//! Bytecode VM backend.

pub struct BytecodeModule {
    pub code: Vec<u8>,
    pub constants: Vec<BytecodeConst>,
}

pub enum BytecodeConst { Int(i128), Uint(u128), Float(u64), Str(Vec<u8>), Bool(bool) }

impl CodegenBackend for BytecodeBackend {
    fn compile_module(&self, ctx: &TyCtx, instances: &[MonoInstance]) -> CompiledModule;
}

pub struct BytecodeBackend;
impl BytecodeBackend { pub fn new() -> Self; pub fn emit_bytecode(&self, instances: &[MonoInstance]) -> BytecodeModule; }
```

---

### Crate 4: `glyim-db`

**Directory:** `crates/glyim-db/`

**Cargo.toml:**
```toml
[package]
name = "glyim-db"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-vfs = { workspace = true }
glyim-interner = { workspace = true }
glyim-syntax = { workspace = true }
glyim-hir = { workspace = true }
glyim-type = { workspace = true }
glyim-thir = { workspace = true }
glyim-mir = { workspace = true }
glyim-diag = { workspace = true }
salsa = { workspace = true }

[features]
mock = []
```

**Contract:**

```rust
//! Incremental compilation database. Owns TyCtx.

#[salsa::db]
pub trait GlyimDb: salsa::Database {
    fn vfs(&self) -> &Vfs;
    fn interner(&self) -> &Interner;
    fn kw(&self) -> &Kw;
}

/// The concrete database. Owns the frozen TyCtx after typeck.
pub struct Database {
    runtime: salsa::Runtime,
    vfs: Vfs,
    interner: Interner,
    kw: Kw,
}

#[salsa::input]
pub fn source_text(db: &dyn GlyimDb, file_id: FileId) -> Arc<String>;

#[salsa::tracked]
pub fn parse(db: &dyn GlyimDb, file_id: FileId) -> Arc<Parse>;

#[salsa::tracked]
pub fn hir(db: &dyn GlyimDb, file_id: FileId) -> Arc<Hir>;

#[salsa::tracked]
pub fn typecheck(db: &dyn GlyimDb, file_id: FileId) -> Arc<TypeckResult>;

#[salsa::tracked]
pub fn lower_mir(db: &dyn GlyimDb, file_id: FileId) -> Arc<Vec<(Name, Body)>>;
```

---

### Crate 5: `glyim-cli`

**Directory:** `crates/glyim-cli/`

**Cargo.toml:**
```toml
[package]
name = "glyim-cli"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-db = { workspace = true }
glyim-codegen-llvm = { workspace = true }
glyim-diag = { workspace = true }
clap = { workspace = true }
miette = { workspace = true }
```

**Contract:**

```rust
//! Command-line interface.

pub fn run() -> Result<(), Box<dyn std::error::Error>>;
```

---

### Crate 6: `glyim-lsp`

**Directory:** `crates/glyim-lsp/`

**Cargo.toml:**
```toml
[package]
name = "glyim-lsp"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-db = { workspace = true }
glyim-diag = { workspace = true }
tokio = { workspace = true }
async-lsp = { workspace = true }
tower = { workspace = true }
```

**Contract:**

```rust
//! Language Server Protocol implementation.

pub async fn run_lsp() -> Result<(), Box<dyn std::error::Error>>;
```

---

### Crate 7: `glyim-runtime`

**Directory:** `crates/glyim-runtime/`

**Cargo.toml:**
```toml
[package]
name = "glyim-runtime"
edition.workspace = true
version.workspace = true

[dependencies]
```

**Contract:**

```rust
//! Minimal runtime support (panic handler, allocator shim, etc.)
//! For v0.1.0, this is essentially empty.
```

---

### Crate 8: `glyim-test`

**Directory:** `crates/glyim-test/`

**Cargo.toml:**
```toml
[package]
name = "glyim-test"
edition.workspace = true
version.workspace = true

[dependencies]
glyim-db = { workspace = true }
glyim-diag = { workspace = true }
insta = { workspace = true }
```

**Contract:**

```rust
//! Integration test infrastructure.

/// Run a compilation snapshot test.
pub fn check_snapshot(source: &str, name: &str);

/// Run a compilation and expect errors.
pub fn expect_errors(source: &str, expected_count: usize);
```

---

# Dispatch Order Summary

```
╔══════════════════════════════════════════════════════════════╗
║  WAVE 0: Subplan 0 — Foundation                             ║
║  Dispatch immediately. Blocks all others.                    ║
╚══════════════════════════════════════════════════════════════╝
                            │
                            ▼
╔══════════════════════════════════════════════════════════════╗
║  WAVE 1: Subplan A, B, C — all in parallel                  ║
║  • A needs: S0 only                                         ║
║  • B needs: S0 + A::glyim-syntax contract (can start with   ║
║    stub while A publishes contract)                          ║
║  • C needs: S0 only                                         ║
╚══════════════════════════════════════════════════════════════╝
                            │
                            ▼
╔══════════════════════════════════════════════════════════════╗
║  WAVE 2: Subplan D, E, F — partially parallel               ║
║  • D needs: S0 + A::syntax contract + C::type contract      ║
║  • E needs: S0 + C::type contract + D::thir contract        ║
║  • F needs: S0 + C::type contract + E::mir contract         ║
║  D can start as soon as C publishes.                        ║
║  E starts after D publishes thir contract.                   ║
║  F starts after E publishes mir contract.                    ║
╚══════════════════════════════════════════════════════════════╝
                            │
                            ▼
╔══════════════════════════════════════════════════════════════╗
║  WAVE 3: Subplan G — Backends & Integration                 ║
║  Needs ALL prior subplans complete and green.                ║
╚══════════════════════════════════════════════════════════════╝
```

**Critical Path:** S0 → SC → SD → SE → SF → SG (approximately 16–20 weeks total).

**Maximum Parallelism:** S0 + (A‖B‖C) + (D‖E‖F, staggered) + G — effectively ~4 concurrent agents during Wave 1, ~3 during Wave 2.

**Contract Publication Gates:** Each subplan must publish stubs and contracts *before* downstream subplans need them. A "contract published" signal from one subplan unlocks the dependent subplan — implementation can proceed independently after that point.
