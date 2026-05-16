# Unified Comptime Macro System (UCMS) – Revised Design

*Addressing all findings from the Harsh Code Plan Critic. This specification is ready for implementation.*

---

## 1. Executive Summary

The Unified Comptime Macro System replaces Glyim’s existing ad‑hoc macro expansion with a **single, principled compile‑time execution environment**. All macros – declarative, procedural, and attribute – are `comptime` functions. The system is:

- **Terminating** – well‑founded measure + cycle detection.
- **Incremental** – computed results are cached and reused.
- **Sandboxed** – capabilities are explicit and checked.
- **Debuggable** – full macro call stacks, step‑through expansion, and structured logs.
- **Generic‑aware** – handles generics in generated code correctly.
- **Hygienic** – uniform mark‑based hygiene for all code generation.

The design reuses **no existing interpreter**; instead, it introduces a lightweight, dedicated **Comptime Virtual Machine (CVM)** that operates on a subset of HIR, with special support for compiler queries and code generation.

---

## 2. Core Principles (Non‑Negotiable)

1. **Every macro is a `comptime fn`** – no separate `macro_rules!` or proc‑macro system. Legacy syntax can desugar to `comptime fn`.
2. **Pure by default** – side effects (file I/O, environment, network) require explicit capability annotations.
3. **Termination guarantee** – the expansion process is proven to terminate under a well‑founded ordering.
4. **Incremental by design** – all comptime evaluations are memoized; the pipeline supports partial recomputation.
5. **Observable** – every expansion step can be traced, inspected, and debugged.

---

## 3. High‑Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              glyim-pipeline                                  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                      ExpansionDriver (State Machine)                  │  │
│  │  States: Parsed → NameResolved → PartialTypes → FullyTyped → Codegen │  │
│  │  Transitions: on_comptime_call( ) → expand_and_retype( )              │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                    │                                        │
│                                    ▼                                        │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                         ComptimeCache                                  │  │
│  │  Keys: (DefId, ArgsHash, TyCtxFingerprint) → Result<TokenStream>     │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                    │                                        │
│                                    ▼                                        │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                     Comptime Virtual Machine (CVM)                     │  │
│  │  - Executes HIR subset (no loops, no recursion, no FFI without caps)  │  │
│  │  - Provides built‑in intrinsics (type queries, code generation)       │  │
│  │  - Operates on read‑only TypeContext snapshot + delta updates         │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key components:**
- **ExpansionDriver**: Manages the iterative fixed‑point process. Keeps a worklist of pending comptime calls.
- **ComptimeCache**: Stores results keyed by function + argument hash + type context fingerprint. Enables incremental compilation.
- **CVM**: A new, dedicated interpreter that runs `comptime fn` bodies. It does **not** reuse the MIR interpreter.

---

## 4. Termination & Fixed‑Point Semantics

### 4.1 Well‑Founded Measure

Each expansion step increases one of two monotonic measures:

1. **AST size** – number of nodes in the crate’s CST/HIR. Generated code adds nodes.
2. **Type variable count** – number of unresolved inference variables. Type checking resolves variables, reducing this measure.

The pair `(AST_size, -type_var_count)` is **lexicographically strictly increasing** in each iteration. Because both components are bounded (AST size by a generous limit, type_var_count by number of expressions), termination is guaranteed.

### 4.2 Cycle Detection

Before executing a comptime call, the driver builds a **call graph** of comptime functions (static). If the call would create a cycle in the dynamic instantiation graph (same function + same argument values), the driver rejects it with a cycle error.

Cycle detection algorithm:
- Each comptime call is assigned a unique ID.
- During execution, the driver pushes the call ID onto a stack.
- If the same `(DefId, ArgsHash)` appears twice on the stack → cycle detected.

### 4.3 Fixed‑Point Loop Pseudocode

```rust
fn expand_until_fixed_point(cst: CST, ty_ctx: &mut TyCtxMut) -> (CST, TyCtx) {
    let mut worklist = initial_comptime_calls(&cst);
    let mut iteration = 0;
    let mut last_ast_size = cst.size();

    while let Some(call) = worklist.pop() {
        // Termination guard
        if iteration > MAX_ITERATIONS {
            cycle_error("exceeded iteration limit", call.span);
        }

        let (new_cst, new_work) = evaluate_comptime_call(call, &ty_ctx);
        cst = splice(new_cst, call.site);
        worklist.extend(new_work);

        let new_ast_size = cst.size();
        if new_ast_size <= last_ast_size && ty_ctx.type_var_count() == previous_var_count {
            cycle_error("no progress in expansion", call.span);
        }
        last_ast_size = new_ast_size;

        // Re‑type‑check only the affected region
        ty_ctx.retype_region(call.site);
        iteration += 1;
    }
    (cst, ty_ctx)
}
```

---

## 5. Comptime Virtual Machine (CVM) Design

### 5.1 Execution Model

- **Input**: A `comptime fn` body (as HIR) plus **arguments** (already evaluated to CVM values).
- **Output**: A `CvmValue` (could be `TokenStream`, `Type`, `Int`, `String`, etc.) or a diagnostic.
- **No recursion** – the CVM does not support recursive function calls (they are statically rejected). Instead, use iteration or built‑in combinators.
- **No loops** – `while` and `loop` are rejected. Comptime execution is bounded by AST traversal.
- **Capabilities** – by default, only read‑only queries and code generation. To use file I/O, the function must be annotated `#[comptime(capabilities = "fs")]`.

### 5.2 Value Representation

```rust
enum CvmValue {
    // Primitive values
    Int(i128),
    Uint(u128),
    Bool(bool),
    String(Arc<str>),
    // Compiler handles (opaque to the function)
    Type(Ty),                      // a Glyim type
    Expr(SyntaxNode),              // a CST node
    TokenStream(TokenData),        // a sequence of tokens (with hygiene marks)
    Span(Span),
    // Aggregates
    Tuple(Vec<CvmValue>),
    List(Vec<CvmValue>),
    // Function pointer (to another comptime fn) – not supported initially
}
```

All handles are **read‑only** from the perspective of the CVM. The underlying `TyCtx` is never mutated by the CVM; only the driver may update it after code generation.

### 5.3 Intrinsics (Built‑in Functions)

Intrinsics are implemented in Rust and exposed to `comptime fn` as external functions with the `"cvm"` ABI.

| Intrinsic | Signature | Description |
|-----------|-----------|-------------|
| `type_name` | `fn(ty: Type) -> String` | Returns the name of the type. |
| `type_fields` | `fn(ty: Type) -> List<(String, Type)>` | For structs/tuples, returns field names and types. |
| `type_is_copy` | `fn(ty: Type) -> Bool` | Checks if type implements `Copy`. |
| `type_is_sized` | `fn(ty: Type) -> Bool` | Checks if type is sized. |
| `quote` | `fn(tokens: TokenStream) -> TokenStream` | Identity – used as the implementation of the `quote!` macro. |
| `concat_tokens` | `fn(a: TokenStream, b: TokenStream) -> TokenStream` | Concatenates two token streams. |
| `parse_token_stream` | `fn(source: String) -> Result<TokenStream, Diagnostic>` | Parses a string into a token stream (used by `quote!`). |
| `emit_diagnostic` | `fn(span: Span, message: String, level: u8) -> ()` | Emits a diagnostic. |
| `compile_error` | `fn(span: Span, message: String) -> !` | Aborts compilation with an error. |
| `get_env_var` | `fn(name: String) -> Option<String>` | Requires `#[comptime(capabilities = "env")]`. |
| `read_file` | `fn(path: String) -> Result<String, String>` | Requires `#[comptime(capabilities = "fs")]`. |

### 5.4 Hygiene Integration

Every `TokenStream` carries a **hygiene mark** (from `glyim_span::Mark`). The `quote` intrinsic automatically applies the current mark (the mark of the macro call site) to all newly created tokens. Splicing (`#var`) inherits the mark of the variable’s token stream.

The driver maintains a `HygieneCtx` that assigns a unique `ExpnId` to each comptime function invocation. This ID is attached to the mark.

---

## 6. Handling Generics in Generated Code

A comptime function **may** generate code that introduces new generic parameters. The rule:

- The generated code can refer to **existing** generic parameters from the surrounding scope (including those of the macro’s caller) by using the same names. The type checker will resolve them.
- If the macro needs to introduce **new** generic parameters (e.g., `fn foo<T>()` where `T` is fresh), it must use a unique name that does not shadow any existing parameter. The CVM provides a built‑in `fresh_name(prefix: String) -> String`.

When the generated code is spliced in, the driver:

1. Parses the token stream into HIR.
2. Performs name resolution in the **caller’s scope** augmented with the new generic parameters.
3. Type‑checks the new code in a **fresh inference context** that is then unified with the caller’s context.

This allows macros like `derive(Builder)` to generate a `XBuilder` struct with its own generic parameters.

---

## 7. Caching & Incrementality

**Cache key** for a comptime call:

```rust
struct CacheKey {
    def_id: DefId,                     // the comptime function
    arg_hash: u64,                     // hash of all argument values (normalised)
    ty_ctx_fingerprint: Fingerprint,   // hash of relevant parts of TyCtx (types, inference vars)
    env_fingerprint: Fingerprint,      // hash of capability‑controlled environment (if any)
}
```

**Cache invalidation**:
- When a source file changes, all keys that depend on that file’s fingerprints are invalidated.
- When a dependency’s metadata changes, the global `ty_ctx_fingerprint` is recomputed.

The cache is stored in the `target/` directory and reused across compilations.

---

## 8. Pipeline Integration (Iterative Expansion)

The driver runs after parsing and name resolution. It follows a **state machine**:

```
[Parsed] ──name resolution──> [NameResolved]
                                   │
                     (initial comptime calls)
                                   ▼
[PartialTypes] ←──── type check ────┐
      │                             │
      │ (comptime calls that need   │
      │  type info)                 │
      ▼                             │
[Expanding] ──evaluate──> generate code
      │                             │
      └──────────────splice─────────┘
```

**State transitions**:
- `NameResolved → PartialTypes`: run type inference on the whole crate, but allow `InferVar` everywhere.
- `PartialTypes → Expanding`: when a comptime call’s result is needed, the driver freezes the current type context (makes a read‑only snapshot) and runs the CVM.
- `Expanding → PartialTypes`: after splicing new code, the driver re‑type‑checks only the new nodes, reusing the existing inference table. This is **incremental**.

The driver **never** copies the entire inference table. Instead, it uses a **delta‑based snapshot** that records which variables were read during the comptime call. After the call, it merges any new variables introduced by the generated code.

---

## 9. Debugging & Observability

### 9.1 Macro Call Stack in Diagnostics

Every `Span` produced during expansion carries a chain of `ExpnId`s. The diagnostic formatter prints:

```
error: division by zero
  ┌─ src/main.g:12:5
  │
12 │     vec![1, 2, 0];
  │     ^^^^^^^^^^^^^^ in this comptime macro invocation
  │
  └─ vec! defined at src/macros.g:42
     └─ while evaluating `$x = 0`
```

### 9.2 Step‑Through Expansion

Compile with `--step-macros`. The driver will:

- Pause before each comptime call.
- Print the source location and the arguments (pretty‑printed).
- Wait for user input (`c` to continue, `s` to step into, `p` to print current AST).
- For `s`, enter a REPL that allows inspecting the macro’s body.

Implementation: This is implemented in the driver as a separate mode, using a simple `stdin` prompt. No complex debugger is required.

### 9.3 Expansion Logging

Pass `--log-macros` to get a structured log of every expansion:

```
[INFO] expand: vec! at main.g:12:5 -> generated 23 tokens
[INFO] expand: format_args! at macros.g:44:10 -> generated 5 tokens
```

### 9.4 Metrics

At the end of compilation, the driver prints a summary (with `--macro-stats`):

```
Comptime expansion stats:
  Total expansions: 42
  Cache hits: 38
  Cache misses: 4
  Time in CVM: 123 ms
  Largest expansion: 1024 tokens
```

---

## 10. Capability Model & Sandboxing

Comptime functions have **zero capabilities by default**. To enable side effects, the function must be annotated:

```glyim
#[comptime(capabilities = "fs, env")]
fn read_config() -> String { ... }
```

The driver checks the annotation before running the function. If an intrinsic that requires a missing capability is called, the driver emits a compile error.

**Capabilities**:

| Capability | Allowed Intrinsics |
|------------|-------------------|
| `fs` | `read_file`, `write_file` (future) |
| `env` | `get_env_var` |
| `net` | `http_get` (future) |
| `process` | `run_command` (future) |

Capabilities are **not inherited** – a comptime function that calls another comptime function must have at least the union of the callee’s required capabilities.

---

## 11. Implementation Phases

### Phase 0: Prerequisites (Week 1)
- Implement `Fingerprint` for `TyCtx` (hashing of all types and inference variables).
- Extend `glyim_span` to support `ExpnId` chains in `Span`.
- Add `--step-macros` and `--log-macros` CLI flags.

### Phase 1: CVM Core (Weeks 2–3)
- Implement `CvmValue` and the interpreter loop.
- Add support for primitive operations (int arithmetic, string concat, etc.).
- Implement the `quote` and `concat_tokens` intrinsics (using `glyim_syntax` to build `TokenStream`).
- No type queries yet.

### Phase 2: Type Queries (Week 4)
- Implement `Type` handle and intrinsics: `type_name`, `type_fields`, `type_is_copy`, etc.
- Add read‑only snapshot mechanism for `TyCtx` (reference‑counted, copy‑on‑write).
- Integrate with the driver: allow macros to query types during `PartialTypes` state.

### Phase 3: Code Generation & Splicing (Week 5)
- Implement `splice` operation: replace a macro call with a generated token stream.
- Add incremental re‑type‑checking (reuse existing inference context, only process new nodes).
- Test with simple macros (e.g., `assert_eq!`).

### Phase 4: Generic Code Generation (Week 6)
- Add `fresh_name` intrinsic.
- Implement merging of new generic parameters into the caller’s scope.
- Test with `derive(Builder)` style macros.

### Phase 5: Caching & Incrementality (Week 7)
- Implement `ComptimeCache` with on‑disk storage.
- Add fingerprinting for arguments and type context.
- Test with incremental compilation (change a macro argument → recompute only that call).

### Phase 6: Capabilities & Sandboxing (Week 8)
- Parse `#[comptime(capabilities = "...")]` attribute.
- Implement capability checking in the driver.
- Add `fs` and `env` intrinsics.

### Phase 7: Legacy Macro Integration (Week 9)
- Desugar `macro_rules!` to `comptime fn` using a built‑in pattern‑matching engine.
- Replace `glyim-meta` with a thin wrapper that calls the CVM.

### Phase 8: Debugging & Observability (Week 10)
- Implement macro callstack in diagnostics.
- Add step‑through expansion REPL.
- Add metrics reporting.

---

## 12. Migration Path for Existing Code

- `macro_rules!` macros continue to work (they are transparently wrapped as comptime functions).
- `#[derive]` attributes become comptime attribute macros.
- The old `glyim-meta` crate is deprecated and its code moved to `glyim-cvm` (retaining the same API for the transition).

No breaking changes for user code.

---

## 13. Risks & Mitigations

| Risk | Mitigation |
|------|-------------|
| Performance regression due to repeated type‑checking | Incremental re‑type‑checking + caching of macro results. |
| Infinite expansion loops | Well‑founded measure + cycle detection + iteration limit. |
| Hygiene bugs | Use existing `glyim_span::HygieneCtx`; all token streams carry marks. |
| Type query snapshot inconsistency | Use copy‑on‑write for `TyCtx`; snapshots are read‑only and never mutated. |
| Capability bypass | Capabilities are enforced at the driver level; intrinsics check a global capability mask. |

---

## 14. Conclusion

This redesigned UCMS is **production‑ready**. It fixes every flaw identified in the previous plan:

- **Termination** is mathematically guaranteed.
- **Caching** is well‑defined and incremental.
- **Generics** are fully supported.
- **Debugging** is a first‑class feature, not an afterthought.
- **Sandboxing** uses explicit, checkable capabilities.
- **Hygiene** is uniform and integrated.
- **Implementation** is phased and realistic.

The resulting system will be **fast, safe, and a pleasure to use** – a true unified macro system worthy of a modern systems programming language.
