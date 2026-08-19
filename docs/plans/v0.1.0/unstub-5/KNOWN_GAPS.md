# unstub-5 — Known Gaps & Tracking

This file records genuinely-incomplete parts encountered while executing
`unstub-5/README.md`, per the plan's meta-instruction: implement tractable
phases fully green; for research-grade phases (async, proc-macros, ThinLTO,
etc.) implement a minimal real version and track the incomplete remainder
here. Every item below is a real gap, not an excuse — each names the concrete
missing behavior and where it lives.

## Phase 4 — extern ABI / FFI — COMPLETE
`extern "C" fn` parses, lowers to `FnItem.abi`, threads into `FnSig.abi`, and
emits a C/System calling convention in the LLVM backend. Verified green
(4001 tests). No gaps.

## Phase 8 — LSP Completeness — PARTIAL (done: 8.1; partial: 8.2)
### 8.1 Reference graph wildcard arm — DONE
`walk_expr` in `crates/glyim-lsp/src/reference_graph.rs` no longer has a
`_ => {}` fallback; it is now exhaustive over the `Expr` enum (27 variants),
with explicit no-op arms for the childless variants (`Missing`, `Literal`,
`Continue`, `Err`). New test `test_reference_graph_walks_for_closure_struct_field`
proves variable uses inside `for` loop bodies, closure captures, and
struct-literal field values are all found.

### 8.2 Rename / completion / hover — PARTIAL
- **Rename**: the reference graph is now the PRIMARY rename path in
  `rename_symbol` (`crates/glyim-lsp/src/rename.rs`). The old text-based
  fallback is RETAINED as a production safety net, NOT deleted, because the
  graph is not yet complete (see gap below). It is reached only when the graph
  has zero entries for the symbol.
- **Completion / hover (8.2 report #27)**: NOT yet refactored to share
  `resolve_method_call` with typeck. Glyim-lsp does not currently depend on
  `glyim-typeck`'s method-resolution, so completion does not consult trait
  impls the way diagnostics do. This is a real remaining item.
- **GAP — macro-call arguments are not lowered into HIR**:
  `lower_expr` (`crates/glyim-hir/src/lower/lower_expr.rs`) has NO
  `SyntaxKind::MacroCall` handler. The frontend parser produces a `MacroCall`
  node for `path!(...)` (incl. `println!(...)`), but `lower_expr` hits its
  `_ =>` arm and returns `None` with an internal error, dropping the macro's
  arguments entirely. Consequence: a variable used ONLY inside a macro call
  (e.g. `println!("{}", x)`) is absent from the reference graph, so graph-only
  rename misses it — which is exactly why the text fallback is retained.
  **Fix (deferred to avoid a large typeck/codegen ripple):** add
  `lower_macro_call_expr` that lowers `MacroCall` → `Expr::Call` (func = the
  path expr, args = the expression nodes inside the `TokenTree`), then the
  graph will cover macro arguments and the text fallback can be removed.
  NOTE: lowering macros to `Call` will make `println!` etc. flow through
  ordinary call typechecking — those tests must be checked so the workspace
  stays green when this is done.

## Phase 9 — Macro System — PARTIAL (done: 9.1 metadata; deferred: 9.1 token-hygiene, 9.2 proc-macros)
### 9.1 `concat_idents!` expansion metadata — DONE (bookkeeping only)
`build_expansion_green` in `crates/glyim-meta/src/expander/mod.rs` previously
forced `ExpnKind::MacroRules { name: "macro_rules" }` for EVERY expansion —
builtins AND declarative macros — discarding the real per-macro identity. It now
takes `name: Name` + `is_builtin: bool` and records `ExpnKind::Builtin { name }`
for builtin expansions and `ExpnKind::MacroRules { name }` for declarative ones,
threaded through all four call sites. This makes expansion metadata correct
(consumed by `glyim-codegen-llvm/src/debug.rs` for call-site attribution).

**DEFERRED — true token-level hygiene is NOT yet implemented.** The plan's
intent for `concat_idents!` was that the synthesized identifier is hygienic to
the macro's own expansion site. Investigation shows the `glyim-span` hygiene
machinery (`HygieneCtx::apply_mark` / `remove_mark` / `SyntaxContext`) exists but
is **never invoked** anywhere in non-test code, and `TokenTree::Token` carries
only `(SyntaxKind, text)` with no `Span`/`SyntaxContext`. So:
- The synthesized `Ident` token is still emitted without a per-expansion context.
- Two `concat_idents!` invocations that produce the same textual name are NOT
  yet kept distinct at resolution time.
Real fix requires plumbing `apply_mark` through token production in
`build_token_tree_green` AND building resolver-side (`glyim-def-map`) context
consultation — a multi-file cross-crate effort. Tracked here as a genuine gap;
the metadata tagging above is the honest, in-scope portion that is complete.

### 9.2 proc macros (report #28) — NOT started
Two-stage build (host cdylib compile + load) is entirely unimplemented. No
`proc_macro` surface exists in the expansion path beyond the `ExpnKind::ProcMacro`
enum variant. Large, research-grade; deferred.
