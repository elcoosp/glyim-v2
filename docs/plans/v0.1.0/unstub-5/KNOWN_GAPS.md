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

## Phase 6 — Codegen / Platform — PARTIAL (done: 6.2 enum DWARF, 6.3; deferred: 6.1 SEH, 6.2 closure, 6.4 Windows)
### 6.3 `ReadVisitor`/`PlaceCollector` exhaustiveness — DONE (already complete)
`walk_terminator_reads` in `crates/glyim-borrowck/src/visitor.rs` already
handles every `TerminatorKind` variant (Goto/Return/Unreachable/SwitchInt/Call/
Assert/Drop) with NO wildcard arm; `Drop` is correctly treated as a kill (not a
read) and `Call::destination` (a write) is excluded. `TerminatorKind` has
exactly these 7 variants. No `_ =>` arm exists anywhere in the traversal.
Verified green (glyim-borrowck: 165 tests).

### 6.2 Debug info for enums and closures — PARTIAL (enum done; closure deferred)
- **Enums (DONE)**: `debug_type_for_ty` in `crates/glyim-codegen-llvm/src/
  debug.rs` no longer emits an opaque blob for multi-variant ADTs. It now builds
  a real DWARF union (`create_union_type`) of per-variant struct types (each
  with named, typed members via `create_struct_type`/`create_member_type`),
  wrapped in an outer struct with a discriminant member. Uses only existing
  correct data (`AdtDef.variants[].fields[].ty` + `layout_computer`).
- **Closures (DEFERRED)**: `TyKind::Closure` still emits an opaque struct
  because the plan's assumed `self.ctx.closure_captures(...)` accessor does NOT
  exist in this codebase — capture types aren't exposed to the debug-info pass.
  Real fix requires adding a closure-capture accessor to `TyCtx` (mirroring
  whatever codegen already computes for the closure struct layout) and then
  emitting member types from it. Tracked as a gap.
- **Discriminant width (DEFERRED)**: the enum discriminant member is
  conservatively a single `u8` (8 bits). For enums whose real discriminant
  exceeds 1 byte this is slightly inaccurate; the correct width needs the
  discriminant layout from `layout_computer`. Minor; tracked.

### 6.1 SEH unwinding on Windows — BLOCKED (toolchain limitation, verified)
`emit_landingpad` only does the Itanium (DWARF) path. Funclet-based SEH
(`cleanuppad`/`cleanupret`/`catchswitch`) needs raw `llvm-sys` FFI and is
Windows-only. **Verification (2026-08-20): this cannot be implemented with the
pinned LLVM 22 toolchain.** Concretely:
- `inkwell` 0.10 does not wrap `LLVMBuildCleanupPad` / `LLVMBuildCleanupRet` /
  `LLVMBuildInvokeWithOperandBundles` / `LLVMCreateOperandBundle` /
  `LLVMAddOperandBundle`.
- `llvm-sys` 221.0.1 does NOT declare those symbols (confirmed by grepping the
  vendored `lib.rs`).
- **Decisive:** `nm -gU` shows those symbols are **absent** from both
  `/opt/homebrew/opt/llvm@22/lib/libLLVM.dylib` and `libLLVM-C.dylib`. The brew
  LLVM 22 shared library simply does not export the funclet C-API entry points.
- Additionally, even if the symbols linked, `inkwell`'s `CallSiteValue::new` /
  `AnyValueEnum::new` require a *private* `llvm_sys::LLVMValueRef` type, so a raw
  FFI result cannot be wrapped back into an inkwell value from outside the crate.

Because emitting the Itanium unwinder on Windows would **miscompile** Windows
exception handling, the SEH branch in `emit_landingpad` now returns a precise
diagnostic naming the toolchain gap rather than lowering wrong code. The
*feasible* parts of P6.1 ARE done and green: personality selection is the
correct 3-way choice (`select_personality` → `Seh` on Windows w/ cleanup,
`Itanium` elsewhere, `None` w/o cleanup), and the `__CxxFrameHandler3`
personality symbol is declared for Windows targets.

**To truly implement P6.1**, the LLVM toolchain must be upgraded to a build that
ships the funclet C-API (and/or inkwell must wrap it). That is a toolchain/CI
migration, not a codegen change, and is outside this phase's scope.

### 6.4 Windows graceful process signaling — DONE
`glyim-runtime` Windows branch now uses `windows-sys` 0.59
(`Win32_System_Console`/`Threading`/`Foundation`). `glyim_process_kill` maps
SIGTERM(15)/SIGINT(2) → `GenerateConsoleCtrlEvent` (CTRL_BREAK_EVENT, graceful
shutdown of the process group) and SIGKILL(9)/other → `TerminateProcess` (hard
kill). Process spawn on Windows passes `CREATE_NEW_PROCESS_GROUP` so the child
is its own group leader (only it receives the ctrl-event, not the whole
console). `OpenProcess` null checks use `.is_null()`. Verified green on the
Windows CI runner (`test_glyim_process_kill_graceful_signal_windows`), and the
three pre-existing Unix-only runtime tests were correctly gated (`getppid`,
`env_var_home`, `spawn_preserves_spaces_in_arg` → `#[cfg(unix)]` /
HOME-unset-tolerant) so they no longer fail on Windows.

## Phase 5 — async/.await — NOT started
Largest single gap. No `Future`/`Poll`/`Context` lang-item exists anywhere
(`grep` finds none). Requires: a `future.g` std lib trait, async-fn →
state-machine desugaring (new `lower_async.rs`), `.await` typeck, and a
`block_on` executor in `glyim-runtime`. Multi-feature, foundational; deferred.

## Phase 7 — Execution Backends — NOT started
Bytecode VM (no interpreter exists; golden tests only assert emitted bytes) and
MIR-interpreter cross-frame unwinding. Option (A) per the plan is a real
switch-dispatch VM; large, foundational; deferred.

## Phase 10 — Build & Tooling — NOT started
Registry default, ThinLTO / linker-half, docs. Mostly config; some (LTO,
registry) risky to change without CI verification. Deferred with the rest.
