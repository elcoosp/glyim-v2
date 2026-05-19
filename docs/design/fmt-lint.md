# 🧠 BMAD Creative Intelligence Suite — Final Brainstorm

## Session: Formatter + Linter × UCMS Deep Integration

**Creative Squad Mode Engaged** — All agents, fully grounded in locked contracts + UCMS spec

---

## 📐 Grounding: The UCMS Is The Integration Backbone

The UCMS already provides everything the linter and formatter need to communicate with the compiler. There is no "bridge to build" — there is a **bridge to use**.

```
UCMS Intrinsic          Linter Uses It                    Formatter Uses It
──────────────────────────────────────────────────────────────────────────────
type_name(ty)           "naming convention mismatch"      canonical type formatting
type_fields(ty)         "struct has >N fields"            break struct literals
type_is_copy(ty)        "unnecessary clone"               format move vs copy context
type_is_sized(ty)       "unsized in invalid position"     —
type_is_enum(ty)        "non-exhaustive match"            format match exhaustively
type_variants(ty)       "missing match arm"               —
type_generic_args(ty)   "overly generic"                  break at generic boundaries
fresh_name(prefix)      generate names in auto-fixes      generate temp names
fresh_type_var()        "what if this were different?"    —
parse_token_stream(s)   verify fix suggestions parse      verify formatted output parses
emit_diagnostic(...)    comptime lint output              format violation output
concat_token_streams    build multi-part fixes            build formatted output
quote_tokens            —                                 — (quote! is the formatter's friend)
read_file(path)         read project lint config          read format config
get_env_var(name)       CI-specific lint levels           CI-specific format checks
```

**Every CVM intrinsic is a two-sided coin.** The linter sees one face, the formatter sees the other. This isn't coincidence — it's because both need the same compiler capabilities.

---

## 🧠 CARSON — Brainstorming Coach

### Technique: Asset-Based Ideation + SCAMPER

#### 36 Ideas — Every One Grounded in a Specific CVM Intrinsic or UCMS Mechanism

**`emit_diagnostic` → The Unified Output (Ideas 1-5)**

1. **`emit_diagnostic` IS the lint output API.** Comptime lint rules call `emit_diagnostic(1, "unused variable", span)` — level 1 = Warning. This goes into `DiagSink` via the CVM→`GlyimDiagnostic` path. Same output as built-in lints. Same LSP delivery. Zero new plumbing.

2. **Format violations as level-2 diagnostics.** The formatter calls `emit_diagnostic(2, "reformatted for readability", span)` — level 2 = Note. This means format suggestions appear as informational notes in the IDE, not warnings. The developer isn't nagged — they're informed.

3. **Level-3 (Help) for lint explanations.** When a lint fires, a secondary `emit_diagnostic(3, "consider adding explicit type", span)` provides the help text. This maps to `DiagSeverity::Help` → `GlyimDiagnostic::with_sub(SubDiagnostic { severity: Help, ... })`. The existing `SubDiagnostic` type already supports this.

4. **`compile_error` for hard lint rules.** A `deny`-level lint rule calls `compile_error(span, "this pattern is forbidden in this codebase")`. This aborts compilation — same as a type error. No special handling needed.

5. **Multi-span diagnostics via repeated `emit_diagnostic`.** A lint like "this function returns `Result` but the caller ignores it" needs two spans: the call site (primary) and the function definition (secondary). The comptime lint rule calls `emit_diagnostic` once with the call site, then the driver attaches the secondary span via `MultiSpan::with_secondary`. Or: the CVM provides an `emit_diagnostic_multi` intrinsic.

**Type Query Intrinsics → Type-Aware Linting (Ideas 6-14)**

6. **`type_is_copy(ty)` → "unnecessary `.clone()`" lint.** The comptime lint rule iterates all method calls. If method name is `clone` and `type_is_copy(receiver_type)` is true, emit warning. Uses: `type_is_copy`, `emit_diagnostic`.

7. **`type_fields(ty)` → "god struct" lint.** If `type_fields(ty).len() > 20`, emit "this struct has too many fields — consider decomposition". Uses: `type_fields`, `emit_diagnostic`.

8. **`type_generic_args(ty)` → "type complexity" lint.** If `type_generic_args(ty).len() > 3`, emit "this type is overly generic — consider a type alias". Uses: `type_generic_args`, `emit_diagnostic`.

9. **`type_variants(ty)` → "non-exhaustive match" lint.** Count match arms vs `type_variants(ty).len()`. If arms < variants, emit "missing match arm". This is ONLY possible with type info. Uses: `type_variants`, `emit_diagnostic`.

10. **`type_name(ty)` → "naming mismatch" lint.** If variable is named `count` but `type_name(ty)` is `"bool"`, emit "variable named `count` is `bool` — consider renaming to `is_*`". Uses: `type_name`, `emit_diagnostic`.

11. **`type_is_enum(ty)` → "match on bool" lint.** If scrutinee type is `bool`, suggest `if` instead of `match`. Uses: `type_is_enum` (returns false for bool — actually, need a separate check. But `type_name(ty) == "bool"` works).

12. **`type_is_sized(ty)` → "unsized in local" lint.** If a local variable has an unsized type, that's an error. But the lint can catch it earlier with a clearer message. Uses: `type_is_sized`, `emit_diagnostic`.

13. **`type_fields(ty)` → "field ordering" lint.** Check if struct fields are sorted by size (largest first for alignment). Use `type_fields` to get types, then `type_is_sized` + layout info to sort. Uses: `type_fields`, `type_is_sized`, `emit_diagnostic`.

14. **`type_generic_args(ty)` → "redundant generic" lint.** If a generic arg is the default (e.g., `HashMap<K, V, RandomState>` where `RandomState` is the default), emit "redundant generic argument". Uses: `type_generic_args`, `emit_diagnostic`.

**Fresh Name + TokenStream → Auto-Fix Generation (Ideas 15-20)**

15. **`fresh_name` in lint auto-fixes.** When a lint fix introduces a new binding (e.g., "extract this expression into a variable"), use `fresh_name("tmp")` to generate a hygienic name. The name is deterministic via the `FreshnessStore` — same compilation, same name.

16. **`concat_token_streams` for multi-part fixes.** A lint fix that replaces an expression with a block: `concat_token_streams(quote!({ let ), fresh_name_tokens, quote!( = ), original_expr, quote!(; }), new_expr, quote!(}))`. Uses: `concat_token_streams`, `quote!`, `fresh_name`.

17. **`parse_token_stream` for fix verification.** After building a fix suggestion as a `TokenStream`, call `parse_token_stream(fix_text)` to verify it parses. If it doesn't, don't emit the fix. This prevents malformed auto-fixes. Uses: `parse_token_stream`.

18. **`quote!` for generating type annotations.** A lint that says "add explicit type annotation" can generate the fix: `quote!(let x: #ty = ...)`. The `#ty` splices the `Type` handle, which produces the type's source representation. Uses: `quote!`, `CvmValue::Type`.

19. **`fresh_type_var` for hypothetical fixes.** A lint that suggests "this could be generic" creates a `fresh_type_var()`, substitutes it into the function signature, and checks if the resulting code typechecks. If it does, emit the suggestion. Uses: `fresh_type_var`, typeck.

20. **`TokenStream` round-tripping for formatting.** The formatter produces a `TokenStream` (via `quote!`-like construction). It verifies the output via `parse_token_stream`. If the round-trip fails, the formatter has a bug — roll back. Uses: `parse_token_stream`, `concat_token_streams`.

**Hygiene → Macro-Aware Linting/Formatting (Ideas 21-25)**

21. **Transparency-driven lint suppression.** Built into `LintContext::emit()`: check `span.ctx`. If opaque, suppress style/correctness lints. If semi-transparent, suppress style only. If transparent, lint normally. The `Transparency` enum already exists in `glyim-span`.

22. **`ExpnId` chain → lint attribution.** When a lint fires in macro-generated code, use `Span::call_stack()` (from UCMS Phase 0, A1's work) to attribute the lint to the macro definition. Developer sees: "warning in `vec!` defined at src/macros.gly:42".

23. **Transparency-driven formatting.** The formatter checks `span.ctx` on each token. `Opaque` expansions are formatted as-is (preserving macro output). `Transparent` expansions are formatted like user code. `SemiTransparent` — format the call site, preserve the expansion.

24. **`quote!` marks propagate through fixes.** When a comptime lint rule generates a fix using `quote!`, the tokens automatically carry the current call site's hygiene mark. This means the fix integrates correctly with the existing hygiene context — no name collisions.

25. **`ExpnKind::Builtin` special handling.** The `file!`, `line!`, `include!` builtins are transparent. Lint and format treat their output as user code. The `ExpnKind::Builtin { name }` variant already exists for this purpose.

**Caching + Incrementality → Performance (Ideas 26-30)**

26. **Lint results cached by `ty_ctx_fingerprint`.** The `ComptimeCache` already uses `ty_ctx_fingerprint: u128` as part of the key. Lint results for a given `(DefId, arg_hash, ty_ctx_fingerprint)` are cached. If the type fingerprint hasn't changed, reuse lint results. No re-linting.

27. **Format results cached by source hash.** The formatter's output depends on the source text and config. Cache the formatted output keyed by `xxhash128(source_text + config_bytes)`. If the source hasn't changed, the formatted output is the same. Uses the same `ComptimeCache` infrastructure.

28. **Incremental lint via `FreshnessStore`.** Track which `DefId`s have changed. Only re-lint changed definitions and their dependents. The `FreshnessStore` from UCMS §6 already provides deterministic, persistent counters.

29. **M3 measure in ExpansionDriver.** Add a third measure to the fixed-point loop: `M3 = hash(all lint/format suggestions)`. If lint or format produces different suggestions after re-typeck, the driver continues. This ensures auto-fixes are stable. The measure vector becomes `(M1, M2, M3)`.

30. **`ComptimeCache::dump` for lint/format debugging.** `--dump-cache` already exists. Extend it to show lint cache entries: which lints fired, what their fixes were, whether they were cached hits. Uses the existing `--dump-cache` infrastructure.

**Capabilities → Sandboxing (Ideas 31-34)**

31. **`#[comptime_lint(capabilities = "type_query,diagnostics")]`**. A comptime lint rule declares what it needs. `type_query` = access to type intrinsics. `diagnostics` = can call `emit_diagnostic`. Without these, the lint rule can't inspect types or emit diagnostics — it's a no-op.

32. **Capability propagation for lint helpers.** If lint rule F calls helper G, and G calls `type_fields`, then F must have `type_query` capability. The UCMS §11 propagation rules handle this automatically. No extra work.

33. **`fs` capability for config-reading lints.** A lint rule that reads project-specific config (via `read_file`) needs `capabilities = "fs"`. This is enforced at comptime — the CVM checks the capability mask before executing `read_file`.

34. **No `env` capability by default for lint rules.** Lint rules shouldn't read environment variables unless explicitly declared. This prevents "works on my machine" lints. The capability system enforces this.

**Pipeline Integration (Ideas 35-36)**

35. **Lint as a post-expansion phase in the ExpansionDriver.** After the expansion loop converges (M1 and M2 stable), run lint passes. If lint auto-fixes change the code (via splicing), re-enter the expansion loop. The driver's existing convergence check handles this naturally.

36. **Format as the final phase.** After lint converges, run the formatter. Format changes don't affect type semantics (they're whitespace-only), so they don't trigger re-typeck. But if the formatter adds type annotations (via `annotate_inferred`), that DOES change semantics — treat it like a lint auto-fix.

---

## 🔬 DR. QUINN — Problem Solving

### Root Cause: Why Is Linter↔Typechecker Integration Hard?

```
Five Whys:
  Why can't linters access type information?
    → They run before/during typechecking
      Why do they run before?
        → Because typecheck can fail, leaving incomplete type info
          Why does incomplete type info prevent linting?
            → Because linters assume all-or-nothing access
              Why all-or-nothing?
                → Because there's no partial type access API

UCMS ANSWER: The CVM intrinsics ARE the partial type access API.
  - type_name works on any Ty, even partially resolved ones
  - type_is_copy returns false if unsure (conservative)
  - type_generic_args returns what's available
  - The CVM snapshot mechanism provides a frozen, consistent view
```

### TRIZ: The Formatter-Macro Contradiction (Resolved by UCMS Hygiene)

**Contradiction:** Formatter wants consistent style everywhere, but macro expansions produce code the user didn't write. Reformatting macro output is confusing; not reformatting looks ugly.

**UCMS Resolution:** The `Transparency` field on `ExpnData` already encodes the macro author's intent:

| Transparency | Macro Author's Intent | Formatter's Response | Linter's Response |
|---|---|---|---|
| `Transparent` | "This IS user code" | Format fully | All lints active |
| `SemiTransparent` | "This is partially user code" | Format call site only | Correctness lints only |
| `Opaque` | "This is implementation detail" | Don't touch | Error-level lints only |

This isn't a new classification. The UCMS hygiene system already provides it. The formatter and linter just need to read `span.ctx` → `expn_data.transparency`.

### Systems Thinking: The ExpansionDriver as Orchestrator

The UCMS ExpansionDriver already solves the hard problems. Lint + format just add dimensions:

```
EXISTING UCMS LOOP:
  expand → typeck → measure(M1, M2) → if changed, loop

EXTENDED LOOP:
  expand → typeck → lint → format → measure(M1, M2, M3) → if changed, loop

Where:
  M1 = AST node count (existing)
  M2 = unresolved type vars (existing)
  M3 = hash of lint/format output (NEW)

Convergence: (M1, M2, M3) all stable for 2 consecutive rounds
Cycle detection: if same (M1, M2, M3) seen before → cycle → halt
Stall detection: if M1 stable but M2/M3 oscillating for 3 rounds → emit current state
```

**The driver's existing mechanisms (cycle detection, stall detection, convergence) handle lint/format for free.** No new orchestration needed.

### The Lint-Fix-Re-Typeck Cycle

When a lint auto-fix is applied via splicing:

```
Round N:   lint fires → fix applied via splice() → code changes
           → M1 increases (new AST nodes) → driver continues

Round N+1: re-typeck new code → re-lint → no new lints → M3 stable
           → CONVERGED → output
```

**The inference table merging algorithm (UCMS §8.2) handles the re-typeck.** The splicing algorithm (UCMS §7.2) handles the code replacement. The lint system just produces the fix as a `TokenStream`.

---

## 🎨 MAYA — Design Thinking

### The Developer Experience: Unified Diagnostics

```
$ glyip check

src/main.gly:12:5 [L2001] warning: unused variable `count`
  = note: this variable is of type `i32` and is never read
  = help: prefix with `_` to suppress: `_count`
  = fix: replace `count` with `_count` (MachineApplicable)

src/main.gly:18:1 [L3012] warning: public function missing type annotation
  = note: `process` returns `Result<Vec<User>, AppError>` (inferred)
  = help: consider adding explicit return type for documentation
  = fix: insert `-> Result<Vec<User>, AppError>` (MachineApplicable)

src/main.gly:24:14 [F1001] note: reformatted for readability
  = note: long method chain broken at type boundary `Result<…, …>`
  = fix: apply formatting (MachineApplicable)

3 diagnostics emitted. Run `glyip fix` to apply MachineApplicable fixes.
```

**One output. Lint warnings and format notes are siblings.** The developer doesn't separate "lint issues" from "format issues" — they're all compiler observations.

### The Concrete Architecture

#### New Crate: `glyim-lint`

```
glyim-lint/
├── Cargo.toml
│   # depends on: glyim-diag, glyim-core, glyim-span, glyim-hir,
│   #   glyim-type, glyim-typeck (thir), glyim-mir, glyim-meta,
│   #   glyim-cvm, glyim-def-map, glyim-cache
├── src/
│   ├── lib.rs
│   ├── registry.rs     # LintRegistry, Lint, LintId, LintLevel, LintCategory
│   ├── context.rs      # LintContext — wraps CVM intrinsics + DiagSink
│   ├── driver.rs       # LintDriver — runs passes at pipeline stages
│   ├── passes/
│   │   ├── mod.rs
│   │   ├── hir.rs      # HirLintPass trait + built-in HIR lints
│   │   ├── thir.rs     # ThirLintPass trait + built-in type-aware lints
│   │   ├── mir.rs      # MirLintPass trait + built-in flow lints
│   │   ├── def_map.rs  # DefMapLintPass trait + scope lints
│   │   └── comptime.rs # ComptimeLintPass — executes CVM lint rules
│   ├── rules/
│   │   ├── mod.rs
│   │   ├── unused.rs       # unused variables, imports, functions
│   │   ├── type_aware.rs   # type-directed lints (CVM intrinsics)
│   │   ├── complexity.rs   # cognitive/type complexity
│   │   └── style.rs        # naming, formatting consistency
│   └── cache.rs        # Wraps glyim-cache::ComptimeCache for lint results
```

#### New Crate: `glyim-fmt`

```
glyim-fmt/
├── Cargo.toml
│   # depends on: glyim-syntax, glyim-span, glyim-core, glyim-diag,
│   #   glyim-cvm (for parse_token_stream verification)
├── src/
│   ├── lib.rs
│   ├── config.rs       # FormatConfig
│   ├── formatter.rs    # Formatter — operates on Rowan SyntaxNode
│   ├── writer.rs       # FormatWriter — tracks position, indent, width
│   ├── comment.rs      # Comment classification via SyntaxKind::is_trivia
│   ├── items.rs        # Format rules per SyntaxKind
│   ├── exprs.rs        # Expression formatting (type-directed)
│   ├── types.rs        # Type annotation formatting
│   ├── macros.rs       # Macro hygiene-aware formatting
│   └── verify.rs       # Post-format verification via parse_token_stream
```

#### The LintContext — Unified Access for All Lint Types

```rust
/// The lint context — provides intrinsic access for both built-in and comptime lints.
///
/// BUILT-IN PASSES: Construct this directly, pass &TyCtx as TypeQueryIntrinsic.
/// COMPTIME PASSES: The CVM dispatches intrinsics automatically.
pub struct LintContext<'a> {
    // === Intrinsic layers (the CVM bridge) ===
    type_query: &'a dyn TypeQueryIntrinsic,
    diag_sink: &'a mut DiagSink,
    fresh: &'a mut dyn FreshIntrinsic,
    parse: &'a dyn ParseIntrinsic,

    // === Compiler state (for built-in passes) ===
    hir: &'a CrateHir,
    def_map: &'a CrateDefMap,
    hygiene: &'a HygieneCtx,
    ty_ctx: &'a TyCtx,

    // === Lint configuration ===
    registry: &'a LintRegistry,

    // === Cache (from glyim-cache) ===
    cache: &'a ComptimeCache,

    // === Comptime interpreter (for comptime lint execution) ===
    cvm: Option<&'a mut CvmInterpreter<'a>>,
}

impl<'a> LintContext<'a> {
    /// Emit a lint diagnostic — hygiene-aware.
    ///
    /// This is the ONLY way lint rules produce output.
    /// It checks hygiene, checks lint level, and emits to DiagSink.
    pub fn emit(&mut self, lint: LintId, span: Span, msg: String) {
        let level = self.registry.level(lint);
        if level == LintLevel::Allow { return; }

        // === HYGIENE CHECK (UCMS §5.4) ===
        if !span.ctx.is_root() {
            let expn = self.hygiene.expn_data(span.ctx);
            let cat = self.registry[lint].category;
            match expn.transparency {
                Transparency::Transparent => {},
                Transparency::SemiTransparent => {
                    if cat != LintCategory::Correctness { return; }
                },
                Transparency::Opaque => return,
            }
        }

        let severity = match level {
            LintLevel::Warn => DiagSeverity::Warning,
            LintLevel::Deny | LintLevel::Forbid => DiagSeverity::Error,
            LintLevel::Allow => unreachable!(),
        };

        let lint_def = &self.registry[lint];
        self.diag_sink.emit(GlyimDiagnostic::new(
            ErrorCode { category: ErrorCategory::Lint, number: lint_def.id.to_raw() as u16 },
            severity,
            msg,
            MultiSpan::from_span(span),
        ));
    }

    /// Emit a lint with an auto-fix suggestion.
    ///
    /// The fix is a Suggestion with Applicability.
    /// MachineApplicable → safe to auto-apply via `glyip fix`.
    /// MaybeIncorrect → show but don't auto-apply.
    pub fn emit_with_fix(
        &mut self,
        lint: LintId,
        span: Span,
        msg: String,
        fix: Suggestion,
    ) {
        // Same hygiene check as emit()...
        let lint_def = &self.registry[lint];
        let severity = /* ... */;
        self.diag_sink.emit(
            GlyimDiagnostic::new(
                ErrorCode { category: ErrorCategory::Lint, number: lint_def.id.to_raw() as u16 },
                severity,
                msg,
                MultiSpan::from_span(span),
            )
            .with_suggestion(fix)
        );
    }

    // === Type query access (via CVM intrinsic layer) ===
    // These call the SAME intrinsics that comptime lint rules use.
    // No code duplication. No API divergence.

    pub fn type_name(&self, ty: Ty) -> String { self.type_query.type_name(ty) }
    pub fn type_is_copy(&self, ty: Ty) -> bool { self.type_query.type_is_copy(ty) }
    pub fn type_fields(&self, ty: Ty) -> Vec<(String, Ty)> { self.type_query.type_fields(ty) }
    pub fn type_generic_args(&self, ty: Ty) -> Vec<GenericArg> { self.type_query.type_generic_args(ty) }
    pub fn type_is_enum(&self, ty: Ty) -> bool { self.type_query.type_is_enum(ty) }
    pub fn type_variants(&self, ty: Ty) -> Vec<(String, Vec<(String, Ty)>)> { self.type_query.type_variants(ty) }
    pub fn type_is_sized(&self, ty: Ty) -> bool { self.type_query.type_is_sized(ty) }

    // === Additional queries (not in UCMS spec, needed for lints) ===
    pub fn type_complexity(&self, ty: Ty) -> u32 { self.type_query.type_complexity(ty) }
    pub fn type_of_span(&self, span: Span) -> Option<Ty> { self.type_query.type_of_span(span) }
    pub fn type_is_result(&self, ty: Ty) -> Option<(Ty, Ty)> { self.type_query.type_is_result(ty) }
    pub fn type_is_option(&self, ty: Ty) -> Option<Ty> { self.type_query.type_is_option(ty) }

    // === Freshness (UCMS §6) ===
    pub fn fresh_name(&mut self, prefix: &str) -> String { self.fresh.fresh_name(prefix) }

    // === Parsing (UCMS §5.3) ===
    pub fn parse_token_stream(&self, tokens: &TokenStream) -> Result<SyntaxNode, String> {
        self.parse.parse_token_stream(tokens)
    }
}
```

#### The TypeQueryIntrinsic Trait — The Bridge

```rust
/// Type query intrinsics — the SAME interface used by both:
/// 1. Built-in lint passes (Rust) — implemented directly on TyCtx
/// 2. Comptime lint rules (CVM) — dispatched by the CVM interpreter
///
/// This trait is the SINGLE integration point between linter/formatter
/// and the type system. No other type access is needed.
pub trait TypeQueryIntrinsic {
    // === UCMS §5.3 intrinsics ===
    fn type_name(&self, ty: Ty) -> String;
    fn type_fields(&self, ty: Ty) -> Vec<(String, Ty)>;
    fn type_is_copy(&self, ty: Ty) -> bool;
    fn type_is_sized(&self, ty: Ty) -> bool;
    fn type_is_enum(&self, ty: Ty) -> bool;
    fn type_variants(&self, ty: Ty) -> Vec<(String, Vec<(String, Ty)>)>;
    fn type_generic_args(&self, ty: Ty) -> Vec<GenericArg>;

    // === Extended intrinsics for lint/format ===
    fn type_complexity(&self, ty: Ty) -> u32;
    fn type_of_span(&self, span: Span) -> Option<Ty>;
    fn type_is_result(&self, ty: Ty) -> Option<(Ty, Ty)>;
    fn type_is_option(&self, ty: Ty) -> Option<Ty>;
}

/// Implemented directly on TyCtx for built-in passes.
impl TypeQueryIntrinsic for TyCtx {
    fn type_name(&self, ty: Ty) -> String {
        format!("{}", PrintTy::new(ty, self))
    }

    fn type_fields(&self, ty: Ty) -> Vec<(String, Ty)> {
        match self.ty_kind(ty) {
            TyKind::Adt(adt_id, substs) => {
                let adt_def = self.adt_def(*adt_id);
                adt_def.variants.first()
                    .map(|v| v.fields.iter()
                        .enumerate()
                        .map(|(i, fty)| {
                            let name = self.name_str(Name::from_raw(i as u32)).to_string();
                            (name, *fty)
                        })
                        .collect()
                    )
                    .unwrap_or_default()
            }
            TyKind::Tuple(substs) => {
                self.substitution_args(*substs).iter()
                    .enumerate()
                    .filter_map(|(i, arg)| {
                        if let GenericArg::Ty(t) = arg { Some((format!("_{i}"), *t)) } else { None }
                    })
                    .collect()
            }
            _ => vec![],
        }
    }

    fn type_is_copy(&self, ty: Ty) -> bool {
        self.is_copy(ty)
    }

    fn type_is_sized(&self, ty: Ty) -> bool {
        // If it has no inference variables and no error, assume sized
        !self.ty_flags(ty).contains(TypeFlags::HAS_ERROR)
    }

    fn type_is_enum(&self, ty: Ty) -> bool {
        matches!(self.ty_kind(ty), TyKind::Adt(_, _)) /* && check AdtKind */
    }

    fn type_variants(&self, ty: Ty) -> Vec<(String, Vec<(String, Ty)>)> {
        match self.ty_kind(ty) {
            TyKind::Adt(adt_id, substs) => {
                self.adt_def(*adt_id).variants.iter()
                    .map(|v| {
                        let fields = v.fields.iter()
                            .enumerate()
                            .map(|(i, fty)| (format!("field_{i}"), *fty))
                            .collect();
                        (format!("variant"), fields)
                    })
                    .collect()
            }
            _ => vec![],
        }
    }

    fn type_generic_args(&self, ty: Ty) -> Vec<GenericArg> {
        match self.ty_kind(ty) {
            TyKind::Adt(_, substs) | TyKind::FnDef(_, substs) | TyKind::Closure(_, substs) => {
                self.substitution_args(*substs).to_vec()
            }
            _ => vec![],
        }
    }

    fn type_complexity(&self, ty: Ty) -> u32 {
        fn depth(ty: Ty, ctx: &TyCtx) -> u32 {
            match ctx.ty_kind(ty) {
                TyKind::Adt(_, substs) | TyKind::Tuple(substs) => {
                    let args = ctx.substitution_args(*substs);
                    1 + args.iter()
                        .filter_map(|a| if let GenericArg::Ty(t) = a { Some(depth(*t, ctx)) } else { None })
                        .max()
                        .unwrap_or(0)
                }
                TyKind::Ref(_, inner, _) | TyKind::Slice(inner) | TyKind::Array(inner, _) => {
                    1 + depth(*inner, ctx)
                }
                _ => 1,
            }
        }
        depth(ty, self)
    }

    fn type_of_span(&self, _span: Span) -> Option<Ty> {
        // Requires typeck result mapping — placeholder
        None
    }

    fn type_is_result(&self, ty: Ty) -> Option<(Ty, Ty)> {
        match self.ty_kind(ty) {
            TyKind::Adt(adt_id, substs) => {
                let args = self.substitution_args(*substs);
                // Check if AdtId corresponds to Result type
                // If so, extract (Ok, Err) type args
                None // placeholder
            }
            _ => None,
        }
    }

    fn type_is_option(&self, ty: Ty) -> Option<Ty> {
        match self.ty_kind(ty) {
            TyKind::Adt(adt_id, substs) => {
                let args = self.substitution_args(*substs);
                // Check if AdtId corresponds to Option type
                None // placeholder
            }
            _ => None,
        }
    }
}
```

#### The Comptime Lint Executor

```rust
/// Executes user-defined lint rules via the CVM.
///
/// Comptime lint rules are `comptime fn` values that the CVM interprets.
/// They use `emit_diagnostic` intrinsic for output.
/// They use type query intrinsics for type access.
/// They are sandboxed by the capability system.
pub struct ComptimeLintExecutor<'a> {
    cvm: &'a mut CvmInterpreter<'a>,
    registry: &'a LintRegistry,
}

impl<'a> ComptimeLintExecutor<'a> {
    /// Execute a comptime lint rule for a specific expression.
    ///
    /// The lint rule's signature is:
    ///   fn check(expr: Expr, ty: Type) -> void
    /// It calls emit_diagnostic internally.
    pub fn execute_expr_lint(
        &mut self,
        lint_fn_def_id: DefId,
        expr: &SyntaxNode,
        ty: Ty,
        sink: &mut DiagSink,
    ) {
        // Prepare CVM arguments
        let expr_value = CvmValue::Expr(expr.clone());
        let ty_value = CvmValue::Type(ty);

        // Call the CVM function
        // The CVM dispatches type query intrinsics automatically
        // The CVM dispatches emit_diagnostic to the DiagSink
        let _result = self.cvm.call_function(
            lint_fn_def_id,
            &[expr_value, ty_value],
        );
    }

    /// Execute a comptime lint rule for an entire item.
    ///
    /// The lint rule's signature is:
    ///   fn check(item: Expr, ty: Type) -> void
    /// It calls emit_diagnostic internally.
    pub fn execute_item_lint(
        &mut self,
        lint_fn_def_id: DefId,
        item: &SyntaxNode,
        ty: Ty,
        sink: &mut DiagSink,
    ) {
        let item_value = CvmValue::Expr(item.clone());
        let ty_value = CvmValue::Type(ty);
        let _result = self.cvm.call_function(lint_fn_def_id, &[item_value, ty_value]);
    }
}
```

#### The Formatter with Type-Query Access

```rust
/// The formatter — operates on Rowan CST, uses CVM type intrinsics
/// for type-directed formatting decisions.
pub struct Formatter {
    config: FormatConfig,
    type_query: Option<Box<dyn TypeQueryIntrinsic>>,
    hygiene: Option<HygieneCtx>,
}

impl Formatter {
    /// Format a syntax node and return replacement suggestions.
    ///
    /// Each suggestion is a Suggestion with MachineApplicable applicability.
    /// The LSP can apply these as code actions.
    /// `glyip fmt` applies them and writes the result.
    pub fn format(&self, node: &SyntaxNode) -> Vec<Suggestion> {
        let mut writer = FormatWriter::new(&self.config);
        self.format_node(node, &mut writer);
        self.diff_to_suggestions(node, &writer)
    }

    fn format_node(&self, node: &SyntaxNode, writer: &mut FormatWriter) {
        // === HYGIENE CHECK (UCMS §5.4) ===
        if !self.should_format(node) {
            writer.write_raw(node.text().to_string());
            return;
        }

        match node.kind() {
            SyntaxKind::FnDef => self.format_fn_def(node, writer),
            SyntaxKind::StructDef => self.format_struct_def(node, writer),
            SyntaxKind::CallExpr => self.format_call_expr(node, writer),
            SyntaxKind::BinaryExpr => self.format_binary_expr(node, writer),
            SyntaxKind::MatchExpr => self.format_match_expr(node, writer),
            _ => self.format_default(node, writer),
        }
    }

    /// Should we format this node? Respects UCMS hygiene transparency.
    fn should_format(&self, node: &SyntaxNode) -> bool {
        let span = self.span_of_node(node);
        if span.ctx.is_root() { return true; }

        let hygiene = match &self.hygiene {
            Some(h) => h,
            None => return true,
        };

        let expn = hygiene.expn_data(span.ctx);
        match self.config.macro_expansion_format {
            MacroFormatMode::CallSiteOnly => span.ctx.is_root(),
            MacroFormatMode::RespectTransparency => {
                match expn.transparency {
                    Transparency::Transparent => true,
                    Transparency::SemiTransparent | Transparency::Opaque => false,
                }
            },
            MacroFormatMode::All => true,
        }
    }

    /// Type-directed call expression formatting.
    /// Uses CVM type query intrinsics to decide line breaking.
    fn format_call_expr(&self, node: &SyntaxNode, writer: &mut FormatWriter) {
        let should_break = self.type_query.as_ref().map_or(false, |tq| {
            if let Some(ty) = self.type_of_callee(node, tq.as_ref()) {
                let complexity = tq.type_complexity(ty);
                let generic_count = tq.type_generic_args(ty).len() as u32;
                complexity > self.config.type_complexity_break
                    || generic_count > self.config.generic_break_threshold
            } else {
                false
            }
        });

        if should_break {
            self.format_call_multiline(node, writer);
        } else {
            self.format_call_single_line(node, writer);
        }
    }

    /// Post-format verification using CVM's parse_token_stream intrinsic.
    fn verify_format(&self, formatted: &str) -> bool {
        // Use the CVM's parse_token_stream intrinsic to verify
        // the formatted output parses correctly.
        // This is a safety net — if the formatter introduces a syntax error,
        // we catch it here and roll back.
        self.type_query.as_ref().map_or(true, |_| {
            // parse_token_stream(formatted).is_ok()
            true // placeholder
        })
    }
}
```

---

## ⚡ VICTOR — Innovation Strategist

### The Disruption: Comptime Lint Rules as a Platform

The CVM turns lint rules from "things the compiler team writes" into "things anyone can write and distribute as packages."

**The three-tier lint system:**

```
┌─────────────────────────────────────────────────────────┐
│  TIER 3: Comptime Lint Rules (user-defined)             │
│  Written in Glyim, executed by CVM                      │
│  Access: type query intrinsics + emit_diagnostic        │
│  Capabilities: explicitly declared and propagated       │
│  Distribution: shared via packages (lint-dependencies)  │
├─────────────────────────────────────────────────────────┤
│  TIER 2: Built-in Lint Passes (Rust)                    │
│  Written in Rust, use LintContext + TypeQueryIntrinsic  │
│  Access: full TyCtx, CrateDefMap, HIR, THIR, MIR       │
│  Distribution: shipped with compiler                    │
├─────────────────────────────────────────────────────────┤
│  TIER 1: Compiler Diagnostics (existing)                │
│  Emitted during typeck, borrowck, etc.                  │
│  Access: everything                                      │
│  Distribution: shipped with compiler                    │
└─────────────────────────────────────────────────────────┘
```

**Every tier uses `DiagSink`.** Every tier uses `GlyimDiagnostic`. Every tier appears in the LSP. The developer sees one unified output.

### The Comptime Lint Rule Authoring Experience

```glyim
// A comptime lint rule — written in Glyim, runs at compile time
#[comptime_lint(
    name = "handler_must_return_result",
    level = "deny",
    capabilities = "type_query,diagnostics"
)]
fn check_handler(item: Expr, ty: Type) {
    // Only check functions with the #[handler] attribute
    if !has_attribute(item, "handler") { return; }

    // Use CVM type query intrinsic
    let name = type_name(ty);

    // Check if return type is Result<T, E>
    if !name.starts_with("Result<") {
        // Use CVM emit_diagnostic intrinsic
        emit_diagnostic(span_of(item),
            format("handler must return Result<T, AppError>, found {}", name),
            0  // error level
        );
    }
}
```

**How this integrates with UCMS:**

1. The `#[comptime_lint]` attribute marks this as a lint rule. The HIR stores it with `capabilities: CapabilitySet`.
2. The lint driver discovers it during HIR traversal.
3. When checking a handler function, the driver calls the CVM with `(item: Expr, ty: Type)`.
4. The CVM executes the function. It dispatches `type_name(ty)` to the type query intrinsic (A4's work). It dispatches `emit_diagnostic(...)` to the diagnostic intrinsic (A5's work).
5. The diagnostic goes into `DiagSink`. The LSP serves it. The developer sees it.

**No new CVM intrinsics needed** (beyond what's already specified). The existing type query + `emit_diagnostic` intrinsics are sufficient for comptime lint rules.

### Comptime Lint Rules as Packages

```toml
# glyip.toml
[package]
name = "my-app"

[lint-dependencies]
company-lint-conventions = "2.0"

[lint]
handler_must_return_result = "deny"  # from company-lint-conventions
no_raw_strings = "allow"             # override from default
```

**How this works with UCMS:**

1. `glyip` resolves lint-dependencies like regular dependencies.
2. The CVM loads the lint rule functions from the dependency's compiled representation.
3. The lint executor runs them with declared capabilities.
4. Results go into `DiagSink`.

### The Formatter as CVM Consumer

The formatter doesn't need to be a comptime function — it's a Rust component for performance. But it consumes the SAME type query intrinsics:

```rust
// Built-in formatter (Rust) uses TypeQueryIntrinsic
let tq: &dyn TypeQueryIntrinsic = &ty_ctx;
let complexity = tq.type_complexity(return_type);
if complexity > config.type_complexity_break {
    format_multiline(call_expr);
}
```

```glyim
// Comptime lint rule uses the SAME intrinsics via CVM
let args = type_generic_args(ty);
if args.len() > 3 {
    emit_diagnostic(span, "overly generic type", 1);
}
```

**Same API. Same semantics. Different execution engine.** This is the power of the intrinsic bridge.

---

## 📖 SOPHIA — Storyteller

### The Story: The Convergence Loop

A developer writes:

```glyim
fn process(data: Result<Vec<String>, Error>) -> Option<i32> {
    match data {
        Ok(v) => Some(v.len() as i32),
        Err(_) => None,
    }
}
```

The ExpansionDriver begins its convergence loop:

**Round 1:** Expand → Typeck → Lint → Format → Measure

- **Expand:** No macros. CST unchanged. M1 = 47.
- **Typeck:** `process` returns `Option<i32>`. M2 = 0 (all types resolved).
- **Lint (ThirLintPass):**
  - "Function returns `Option` but input is `Result` — error information is discarded." (uses `type_is_result` on input, `type_is_option` on output)
  - "`v.len() as i32` — potential truncation on 64-bit platforms." (uses `type_name` to know `len()` returns `usize`)
- **Lint (ComptimeLintPass):**
  - Company lint rule fires: "public function missing doc comment." (uses `emit_diagnostic`)
- **Format:**
  - Type-directed: `Result<Vec<String>, Error>` has complexity 3 → format with line break.
  - Output:

```glyim
fn process(
    data: Result<Vec<String>, Error>,
) -> Option<i32> {
    match data {
        Ok(v) => Some(v.len() as i32),
        Err(_) => None,
    }
}
```

- **Measure:** M1 = 49 (format added newlines), M2 = 0, M3 = hash(lints + format).

**Round 2:** M1 changed → re-expand → re-typeck → re-lint → re-format

- **Expand:** No macros. CST has 49 nodes. M1 = 49.
- **Typeck:** Same types. M2 = 0.
- **Lint:** Same lints fire. M3 = same hash.
- **Format:** Already formatted. No changes.
- **Measure:** (49, 0, M3_hash) — same as Round 1.

**CONVERGED.** Output the formatted code + diagnostics.

The developer runs `glyip fix`:

```
$ glyip fix
Applied 2 auto-fixes:
  - Added doc comment to `process`
  - Added `#[allow(truncation)]` annotation (with your confirmation)
2 diagnostics remain (1 warning, 1 note)
```

**One system. One loop. One output.** The ExpansionDriver's fixed-point semantics guarantee that lint auto-fixes and format changes are applied correctly.

---

## 🏆 Synthesis: The Architecture

### How Lint + Format Fit the UCMS 10-Agent Plan

| Agent | UCMS Task | Additional Lint/Fmt Work | Why It Fits |
|-------|-----------|--------------------------|-------------|
| A1 | `glyim-span` + driver | Add M3 measure to driver for lint/format fingerprint | Driver already manages measures |
| A2 | `glyim-syntax` + TokenStream | — | TokenStream is already the fix/format representation |
| A3 | `glyim-cvm` core | — | CvmValue::Expr/Type/Span/TokenStream already exist |
| A4 | Type query intrinsics | Add `type_complexity`, `type_is_result`, `type_is_option` | These ARE the lint/format integration |
| A5 | Diag/IO/freshness intrinsics | Map `emit_diagnostic` levels to `DiagSeverity` | `emit_diagnostic(1,...)` → Warning, etc. |
| A6 | Fragment parser | — | Already used for lint fix + format verification |
| A7 | `glyim-type` enhancements | Implement extended TypeQueryIntrinsic on TyCtx | These methods power both lint and format |
| A8 | `glyim-solve` helpers | `unresolved_type_vars()` → "add annotation" lint | Already planned |
| A9 | Splicing + retype | Lint auto-fixes + format changes applied via `splice()` | Same mechanism as macro expansion |
| A10 | `glyim-cache` | Cache lint results with `ty_ctx_fingerprint` key | Same cache infrastructure |

### Required Changes to Locked Contracts

| Change | Crate | What | Justification |
|---|---|---|---|
| `ErrorCategory::Lint` | `glyim-diag` | New enum variant | Lint diagnostics need their own category |
| `TyCtx` methods | `glyim-type` | `type_complexity()`, `type_is_result()`, `type_is_option()` | Power type-aware linting and type-directed formatting |
| `LintConfig` | `glyip` | New `[lint]` and `[format]` sections in `GlyipToml` | User-configurable lint levels and format settings |

### No Changes Required (Works As-Is)

| Existing System | Lint Use | Format Use | UCMS Mechanism |
|---|---|---|---|
| `GlyimDiagnostic` + `DiagSink` | Emit lint diagnostics | Emit format notes | `emit_diagnostic` intrinsic |
| `Suggestion` + `Applicability` | Auto-fix suggestions | Format replacements | `TokenStream` fixes |
| `MultiSpan` | Cross-reference lints | — | `emit_diagnostic` with secondary spans |
| `SyntaxNode` (Rowan) | Map HIR→CST for spans | Format input | `CvmValue::Expr(SyntaxNode)` |
| `SyntaxKind::is_trivia()` | — | Comment/trivia formatting | Already classified |
| `HygieneCtx` + `Transparency` | Suppress lints in macro code | Skip formatting in macro code | `span.ctx` → `expn_data.transparency` |
| `ExpnData` / `ExpnKind` | Attribute lints to macro def site | — | `ExpnId` chain on `Span` |
| `TyCtx` + `TypeLookup` | Type queries via intrinsic | Type-directed formatting | Type query intrinsics |
| `CrateDefMap` + `ItemScope` | Unused imports, visibility | Import sorting | `type_fields` for struct field queries |
| `thir::Body` | Type-aware lint surface | — | `CvmValue::Expr` for CST-level access |
| `glyim-mir::Body` | Flow-sensitive lint surface | — | MIR traversal |
| CVM Interpreter | Execute comptime lint rules | — | Already planned |
| Type query intrinsics | Primary type access | Primary type access | Already planned (A4) |
| `emit_diagnostic` intrinsic | Lint output for comptime rules | Format violation output | Already planned (A5) |
| `parse_token_stream` | Verify fix suggestions | Verify formatted output | Already planned (A5/A6) |
| `fresh_name` | Generate names in fixes | Generate temp names | Already planned (A5/A10) |
| `concat_token_streams` | Build multi-part fixes | Build formatted output | Already planned |
| `ComptimeCache` | Cache lint results | Cache format output | Already planned (A10) |
| `FreshnessStore` | Lint suppression keys | — | Already planned (A10) |
| ExpansionDriver | Fixed-point loop with M3 | — | Already planned (A1) |
| `splice()` | Apply lint auto-fixes | Apply format changes | Already planned (A9) |
| `TyCtx::snapshot()` / COW | Lint on frozen snapshot | Format on frozen snapshot | Already planned (A7) |
| `ty_ctx_fingerprint` | Cache key for lint results | Cache key for format | Already planned (A7) |
| `total_type_vars_created` | "Inference complexity" lint | — | Already planned (A7) |
| `unresolved_type_vars` | "Add annotation" lint | — | Already planned (A8) |
| Capabilities system | Sandbox comptime lint rules | — | Already planned (Phase 6) |
| `LspState` | Lints appear in IDE for free | Format suggestions in IDE for free | Already implemented |
| `Vfs` | — | Write formatted files | Already implemented |
| `glyim-test` assertions | Lint test infra | Format test infra | Already implemented |

### The Complete Pipeline

```
Source → Lexer → Parser → Rowan CST
                    │
                    ├─→ [FMT: format_syntax] ─→ format suggestions (optional)
                    │
                    ↓
              ┌──────────────────────────────────────────┐
              │     EXPANSION DRIVER (UCMS)               │
              │                                            │
              │  expand → typeck → lint → format → measure │
              │     ↑                         │            │
              │     └───── if M1/M2/M3 changed ┘           │
              │                                            │
              │  Convergence: (M1, M2, M3) stable          │
              │  Cycle: same (M1, M2, M3) → halt           │
              │  Stall: M1 stable, M2/M3 oscillating → emit│
              └──────────────────────────────────────────┘
                    │
                    ↓
              HIR Lowering → Def Map → Typeck → MIR → Borrowck → Opt → Codegen
                              │            │         │
                              │            │         │
                         [LINT: scope] [LINT: type] [LINT: flow]
                                      [FMT: smart]

ALL diagnostics → DiagSink → LSP / CLI
```

### Implementation Priority (Aligned with UCMS Phases)

| Priority | Task | UCMS Phase | Agent |
|---|---|---|---|
| 🔴 P0 | `ErrorCategory::Lint` | Phase 0 (day 1) | — |
| 🔴 P0 | `TypeQueryIntrinsic` trait | Phase 3 (day 3-6) | A4 |
| 🔴 P0 | `LintRegistry` + `LintContext` | Phase 5 (day 3-6) | New |
| 🟡 P1 | `ThirLintPass` + 5 type-aware rules | Phase 6 (day 5-8) | New |
| 🟡 P1 | Hygiene-aware `LintContext::emit` | Phase 0-1 (day 1-2) | A1 |
| 🟡 P1 | `glyim-fmt` core (Rowan) | Phase 5 (day 3-6) | New |
| 🟡 P1 | `ComptimeLintExecutor` | Phase 8 (day 6-10) | A5 |
| 🟢 P2 | Type-directed formatting | Phase 6 (day 5-8) | A7 |
| 🟢 P2 | `MirLintPass` | Phase 6 (day 5-8) | New |
| 🟢 P2 | M3 measure in ExpansionDriver | Phase 8 (day 6-10) | A1 |
| 🟢 P2 | Lint result caching | Phase 7 (day 4-8) | A10 |
| 🔵 P3 | `glyip` lint/fmt config | Phase 9+ | — |
| 🔵 P3 | `fmt --lint-fix` mode | Phase 9+ | A9 |

### The 5 Built-in Type-Aware Lint Rules (P1 Prototype)

```rust
// 1. Unused Result — uses type_is_result intrinsic
struct UnusedResult;
impl ThirLintPass for UnusedResult {
    fn lints(&self) -> Vec<LintId> { vec![UNUSED_RESULT] }
    fn check_thir_expr(&mut self, ctx: &mut LintContext, expr: &thir::Expr) {
        // Check if expression produces a Result that's discarded
        if let Some(ty) = ctx.type_of_span(expr.span) {
            if let Some((ok_ty, err_ty)) = ctx.type_is_result(ty) {
                if !ctx.is_used(expr.id) {
                    ctx.emit_with_fix(UNUSED_RESULT, expr.span,
                        "unused Result value — consider handling the error".into(),
                        Suggestion {
                            message: "add let _ =".into(),
                            replacements: vec![(expr.span, format!("let _ = {}", expr.text))],
                            applicability: Applicability::MaybeIncorrect,
                        });
                }
            }
        }
    }
}

// 2. Unnecessary Clone — uses type_is_copy intrinsic
struct UnnecessaryClone;
impl ThirLintPass for UnnecessaryClone {
    fn lints(&self) -> Vec<LintId> { vec![UNNECESSARY_CLONE] }
    fn check_thir_expr(&mut self, ctx: &mut LintContext, expr: &thir::Expr) {
        if let thir::ExprKind::MethodCall { receiver, method, .. } = &expr.kind {
            if method.as_name().map(|n| n.as_symbol()) == Some(intern("clone")) {
                if let Some(recv_ty) = ctx.type_of_span(receiver.span) {
                    if ctx.type_is_copy(recv_ty) {
                        ctx.emit_with_fix(UNNECESSARY_CLONE, expr.span,
                            "cloning a Copy type is unnecessary".into(),
                            Suggestion {
                                message: "remove .clone()".into(),
                                replacements: vec![(expr.span, receiver.text.to_string())],
                                applicability: Applicability::MachineApplicable,
                            });
                    }
                }
            }
        }
    }
}

// 3. Type Complexity — uses type_generic_args intrinsic
struct TypeComplexity;
impl ThirLintPass for TypeComplexity {
    fn lints(&self) -> Vec<LintId> { vec![TYPE_COMPLEXITY] }
    fn check_thir_expr(&mut self, ctx: &mut LintContext, expr: &thir::Expr) {
        if let Some(ty) = ctx.type_of_span(expr.span) {
            let complexity = ctx.type_complexity(ty);
            if complexity > 4 {
                ctx.emit(TYPE_COMPLEXITY, expr.span,
                    format!("type {} has complexity {} (threshold 4) — consider a type alias",
                        ctx.type_name(ty), complexity));
            }
        }
    }
}

// 4. Non-Exhaustive Match — uses type_variants intrinsic
struct NonExhaustiveMatch;
impl ThirLintPass for NonExhaustiveMatch {
    fn lints(&self) -> Vec<LintId> { vec![NON_EXHAUSTIVE_MATCH] }
    fn check_thir_expr(&mut self, ctx: &mut LintContext, expr: &thir::Expr) {
        if let thir::ExprKind::Match { scrutinee, arms } = &expr.kind {
            if let Some(scrut_ty) = ctx.type_of_span(scrutinee.span) {
                if ctx.type_is_enum(scrut_ty) {
                    let variants = ctx.type_variants(scrut_ty);
                    let matched_count = arms.len();
                    if matched_count < variants.len() {
                        ctx.emit(NON_EXHAUSTIVE_MATCH, expr.span,
                            format!("match covers {} of {} variants",
                                matched_count, variants.len()));
                    }
                }
            }
        }
    }
}

// 5. Missing Type Annotation — uses type_name + unresolved_type_vars
struct MissingPubAnnotation;
impl ThirLintPass for MissingPubAnnotation {
    fn lints(&self) -> Vec<LintId> { vec![MISSING_PUB_ANNOTATION] }
    fn check_thir_expr(&mut self, ctx: &mut LintContext, expr: &thir::Expr) {
        // Check public function signatures for missing type annotations
        // If the return type was inferred (not explicitly written),
        // suggest adding an annotation for documentation purposes
    }
}
```

### The Format-Lint-Fix Convergence Loop

```
$ glyip fix

Round 1: Expand (converged) → Typeck → Lint → Format → Measure
  Lint: 3 warnings (2 MachineApplicable fixes)
  Format: 2 regions reformatted
  M3 = hash(lint_output + format_output)

Round 2: Splice fixes → Re-typeck (incremental) → Re-lint → Re-format → Measure
  Lint: 0 new warnings (all fixed)
  Format: 0 changes (already formatted)
  M3 unchanged → CONVERGED

Applied 2 lint fixes, 2 format changes.
```

The ExpansionDriver's existing convergence logic handles this. No new mechanism needed — just add M3 to the measure vector.

---

Want me to proceed with any of these?
- **Write the `glyim-lint` crate skeleton** with all type signatures matching locked contracts
- **Write the `glyim-fmt` formatter** with Rowan visitor for every `SyntaxKind`
- **Design the `ComptimeLintExecutor`** CVM dispatch in full detail
- **Write the M3 measure** integration into the `ExpansionDriver`
- **Draft the `ErrorCategory::Lint` change request**
