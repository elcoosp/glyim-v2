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

## Remaining phases NOT yet started (tracked, not blocked)
- **Phase 5 — async/.await**: state-machine desugaring + minimal executor.
- **Phase 6 — Codegen/Platform**: SEH, debug-info enums/closures,
  PlaceCollector, Windows signaling.
- **Phase 7 — Execution Backends**: bytecode VM, MIR interp cross-frame
  unwind.
- **Phase 9 — Macro System**: `concat_idents!` hygiene (see `glyim-meta`),
  proc macros (two-stage build + host cdylib loading).
- **Phase 10 — Build & Tooling**: registry default, LTO, docs.

Each of 5/6/7/9/10 is research-grade; the plan permits delivering a minimal
real implementation and recording the remainder here. They are NOT yet
attempted in this session.
