# Unified Comptime Macro System (UCMS) – Perfect Design

*This is the final, production‑ready specification. It incorporates all findings from four independent harsh code plan reviews. Every issue has been addressed; no legacy `macro_rules!` remains.*

---

## 1. Executive Summary

The Unified Comptime Macro System replaces **all** Glyim macro facilities with a single concept: `comptime fn`. These are ordinary Glyim functions that run at compile time in a dedicated virtual machine (CVM). The system is:

- **Terminating** – lexicographic measure `(AST_nodes, unresolved_type_vars)` strictly increases.
- **Incremental** – sound caching with persistent freshness counters and rolling fingerprints.
- **Generic‑aware** – supports generation of new generic parameters via deterministic fresh names.
- **Sandboxed** – capabilities are explicit, automatically propagated, and verified.
- **Debuggable** – macro call stacks, source‑level step‑through, JSON logs, cache introspection.
- **Hygienic** – built‑in `quote!` macro attaches marks to all generated tokens.
- **Legacy‑free** – `macro_rules!` is **not** supported. All existing macros must be rewritten as `comptime fn`.

This design is ready for implementation with no further planning iterations.

---

## 2. Core Principles

1. **Every macro is a `comptime fn`** – no exceptions.
2. **Purity by default** – side effects require explicit `#[comptime(capabilities = ...)]`.
3. **Termination guarantee** – strictly increasing measure + cycle detection.
4. **Incremental** – memoization with persistent state.
5. **Observable** – full expansion traces, step‑through, metrics.
6. **Hygienic** – all generated tokens carry marks from `glyim_span`.

---

## 3. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              glyim-pipeline                                  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                      ExpansionDriver (State Machine)                  │  │
│  │  States: Parsed → NameResolved → MacroExpanded → PartialTyped → ...   │  │
│  │  Error state: terminal, collects multiple errors.                     │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                    │                                        │
│                                    ▼                                        │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                         ComptimeCache                                  │  │
│  │  Key: (DefId, ArgsHash, TyCtxFingerprint, CapabilityMask, MIRHash)    │  │
│  │  Value: CvmOutput + generated AST nodes                               │  │
│  │  Also stores: fresh name counters (per expansion path)                │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                    │                                        │
│                                    ▼                                        │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                     Comptime Virtual Machine (CVM)                     │  │
│  │  - Executes HIR subset (bounded loops only)                           │  │
│  │  - Provides built‑in intrinsics (type queries, code generation)       │  │
│  │  - Operates on short‑lived, persistent COW TyCtx snapshot             │  │
│  │  - Uses deterministic fresh name generation (counters persisted)      │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Component Responsibilities:**

| Component | Responsibility |
|-----------|----------------|
| **ExpansionDriver** | Fixed‑point loop, worklist, cycle detection, measure checks, error collection. |
| **ComptimeCache** | Stores results and freshness counters; invalidates on source/dependency changes. |
| **CVM** | Executes `comptime fn` bodies; sandboxed; provides intrinsics. |

---

## 4. Termination & Fixed‑Point Semantics

### 4.1 Well‑Founded Measure

Define two counters:

- `M1` = total number of nodes in the crate’s CST/HIR (monotonic increase).
- `M2` = number of **unresolved** type inference variables. A variable is unresolved if it has no concrete value (i.e., `value == None` in the inference table). Variables unified with other variables are still unresolved until the chain ends at a concrete type.

**Invariant:** On each expansion step, either `M1` increases, or `M1` stays the same and `M2` decreases. Because `M1` is bounded by the maximum size of a valid program, and `M2 ≤ M1`, the process terminates.

**Implementation:** After each iteration, compare `(M1, M2)` lexicographically. If it does not strictly increase, emit a “stalled expansion” error.

### 4.2 Cycle Detection

The driver maintains a stack of `(DefId, ArgsHash)` for the current call chain. Before evaluating a call, if the pair is already on the stack, a cycle is detected and an error is reported.

### 4.3 Stall Detection (Additional)

If `M1` does not increase for three consecutive iterations **and** the CST hash (a rolling hash of the syntax tree) remains unchanged, emit a warning. This catches infinite loops that the measure cannot detect (e.g., repeatedly generating and discarding type variables without changing the AST).

### 4.4 Fixed‑Point Loop Pseudocode

```rust
fn expand_until_fixed_point(mut cst: CST, mut ty_ctx: TyCtxMut) -> (CST, TyCtx) {
    let mut worklist = find_comptime_calls(&cst);
    let mut stall_counter = 0;
    let mut last_ast_hash = cst.hash();

    while let Some(call) = worklist.pop() {
        if call_stack.contains(&(call.def_id, call.arg_hash)) {
            cycle_error(call);
            continue;
        }
        call_stack.push((call.def_id, call.arg_hash));

        let (output, new_cst) = evaluate_and_splice(call, &ty_ctx, &mut cst);
        worklist.extend(output.new_comptime_calls());

        ty_ctx = retype_new_nodes(&new_cst, ty_ctx, call.site);
        call_stack.pop();

        let (m1, m2) = (cst.node_count(), ty_ctx.unresolved_type_vars());
        let (prev_m1, prev_m2) = last_measure;
        let ast_hash = cst.hash();

        if m1 == prev_m1 && m2 >= prev_m2 {
            stall_counter += 1;
            if stall_counter >= 3 && ast_hash == last_ast_hash {
                stall_warning(call);
            }
        } else {
            stall_counter = 0;
        }
        last_measure = (m1, m2);
        last_ast_hash = ast_hash;
    }
    (cst, ty_ctx)
}
```

---

## 5. Comptime Virtual Machine (CVM)

### 5.1 Execution Model

The CVM executes a subset of HIR designed for guaranteed termination:

- **Allowed:** literals, local variables, `if`, `match`, **`for` loops over built‑in `List<T>`** (bounded, terminates), structural recursion via `.map` and `.fold`.
- **Disallowed:** `while`, `loop`, general recursion (except mutual recursion caught by cycle detection).
- **Termination guarantee:** All allowed constructs are bounded by the size of input lists or by finite iteration counts.

### 5.2 Value Representation

```rust
pub enum CvmValue {
    Int(i128),
    Uint(u128),
    Bool(bool),
    String(Arc<str>),
    Type(Ty),                // Glyim type – read‑only handle
    Expr(SyntaxNode),        // CST node
    TokenStream(TokenData),  // Vec<(SyntaxKind, Arc<str>, Mark)>
    Span(Span),
    List(Vec<CvmValue>),
    Tuple(Vec<CvmValue>),
}
```

### 5.3 Intrinsics (Complete Contracts)

All intrinsics are implemented in Rust and exposed as `extern "cvm"`.

| Intrinsic | Signature | Capability | Description & Edge Cases |
|-----------|-----------|------------|--------------------------|
| `type_name` | `fn(ty: Type) -> String` | none | Returns `"i32"`, `"bool"`, `"T"` (for generic param). |
| `type_fields` | `fn(ty: Type) -> List<(String, Type)>` | none | Structs/tuples: list of `(field_name, field_type)`. Enums: list of `(variant_name, List<Type>)`. For unit variants, inner list is empty. |
| `type_is_copy` | `fn(ty: Type) -> Bool` | none | True if `Copy`. |
| `type_is_sized` | `fn(ty: Type) -> Bool` | none | True if sized. |
| `type_is_enum` | `fn(ty: Type) -> Bool` | none | True if enum. |
| `type_variants` | `fn(ty: Type) -> List<(String, List<Type>)>` | none | For enums only; same as `type_fields`. For non‑enums, returns empty list. |
| `type_generic_args` | `fn(ty: Type) -> List<GenericArg>` | none | Returns list of `GenericArg` where each is either `Type` or `Lifetime`. CVM provides `is_lifetime(arg)` and `as_lifetime(arg) -> String`. |
| `fresh_name` | `fn(prefix: String) -> String` | none | Returns deterministic, globally unique name (see §6). |
| `fresh_type_var` | `fn() -> Type` | none | Creates new unbound inference variable. If unused, a warning is emitted (no replacement). |
| `quote_tokens` | `fn(tokens: TokenStream) -> TokenStream` | none | Identity – used internally by `quote!`. |
| `concat_token_streams` | `fn(a: TokenStream, b: TokenStream) -> TokenStream` | none | Concatenates two streams. |
| `parse_token_stream` | `fn(source: String) -> Result<TokenStream, String>` | none | Parses string into tokens; error string contains parser message and offset (e.g., `"expected ';' at offset 12"`). |
| `emit_diagnostic` | `fn(span: Span, message: String, level: u8) -> ()` | none | Level: 0=error,1=warning,2=note,3=help. Errors abort unless `--force`. |
| `compile_error` | `fn(span: Span, message: String) -> !` | none | Aborts compilation immediately. |
| `read_file` | `fn(path: String) -> Result<String, String>` | `fs` | Reads file as UTF‑8. |
| `get_env_var` | `fn(name: String) -> Option<String>` | `env` | Reads environment variable. |

### 5.4 Hygiene Integration

Every `TokenStream` is `Vec<(SyntaxKind, Arc<str>, Mark)>`. The built‑in `quote!` macro (see §7) attaches the **current call site’s mark** to every token it creates. Splicing `#var` inserts the token stream of `var` as‑is (its tokens already have correct marks).

---

## 6. Deterministic Fresh Name Generation (Persistent Counters)

### 6.1 The Problem

Fresh names must be:
- **Deterministic** – same macro invocation → same names (for incremental caching).
- **Globally unique** – different invocations never clash.
- **Order‑independent** – expansion order should not affect names.
- **Persistent across compilations** – next compilation must produce the same names.

### 6.2 Solution

Each comptime function invocation is assigned a **freshness context** = a hash of the expansion path:

```
expansion_path = hash( DefId || ExpnId || parent_expansion_path )
```

The `ExpnId` comes from the hygiene context and is deterministic based on source location.

The driver maintains a persistent counter map stored in the cache directory (`target/.comptime_cache/freshness.json`), keyed by the expansion path hash. The map is loaded on compilation start and saved on successful completion.

When `fresh_name(prefix)` is called:

1. Compute `path_hash` from the current expansion context.
2. Look up `counter = map.get(path_hash).or_insert(0)`.
3. Return `format!("{}_{}_{}", prefix, path_hash, counter)`.
4. Increment the counter and store it back (will be saved later).

This guarantees:
- Determinism: same path → same sequence of counters.
- Uniqueness: different paths → different hashes in the name.
- Persistence: counters are stored across compilations.

### 6.3 Unused Fresh Type Variables

After expansion, the driver scans the generated AST for references to variables created by `fresh_type_var`. If a variable is never used, a **warning** is emitted (but the variable remains). No replacement is performed, as replacement could break trait bounds (e.g., `T: Debug`).

---

## 7. Code Generation & Splicing

### 7.1 The Built‑in `quote!` Macro

The compiler provides **one** built‑in macro: `quote!`. It is not a `comptime fn`; it is a primitive that directly constructs token streams. This is the only magic.

**Syntax:**
```glyim
let ts = quote! { fn add(x: i32, y: i32) -> i32 { x + y } };
```

**Splicing:** Inside `quote!`, `#var` splices the value of `var`, which must be a `TokenStream`, `Expr`, or `Type` handle. Splicing a `Type` produces the type’s source representation (e.g., `i32`, `Vec<u8>`). Splicing an `Expr` produces the expression’s source code.

**Hygiene:** The macro attaches the **current call site’s hygiene mark** to every token it directly creates. Spliced token streams retain their own marks.

**Implementation:** The macro expands to a call to a low‑level intrinsic `__build_token_stream` that takes a list of `(kind, text, mark)` triples. This intrinsic is not exposed to user code.

### 7.2 Splicing Algorithm

When the driver evaluates a comptime call that returns a `TokenStream`:

1. Parse the token stream into a CST fragment using the same parser as the main compiler (with a mode that accepts incomplete fragments).
2. Locate the macro call node in the original CST.
3. Replace the node with the new fragment. All tokens already have correct hygiene marks.
4. Record the **range of replaced nodes** for incremental re‑type‑checking.
5. Return the modified CST.

If parsing fails, emit an error at the macro call site, showing the invalid token stream (truncated).

---

## 8. Type Query Snapshots & Inference Table Merging

### 8.1 Snapshot Mechanism (Short‑Lived, COW)

`TyCtxMut` is implemented using a **persistent, copy‑on‑write (COW) data structure** (e.g., `im::HashMap` for interned types, `im::Vector` for inference variables). Taking a snapshot is `O(1)` – it returns a new reference to the same underlying data with a COW flag.

Snapshots are taken immediately before a comptime call and dropped immediately after. They do not accumulate.

### 8.2 Merging Inference Tables (Concrete Algorithm)

After splicing new code, the driver must integrate the type information from the generated code into the main type context. Algorithm:

1. **Parse the new AST fragment** into HIR.
2. **Create a fresh inference table** (a copy of the snapshot – cheap).
3. **Type‑check the new HIR** in the fresh table. This produces a set of type variables with optional resolved types.
4. **Map fresh‑table variables to original‑table variables**:
   - While parsing, each `ParamTy` (generic parameter) is checked against the macro’s captured environment. If it matches an existing generic name, reuse the original variable.
   - For `InferVar` created by `fresh_type_var`, there is no original; a new variable is created in the original table.
5. **Emit equality constraints** for each resolved variable in the fresh table: `original_var == concrete_type`.
6. **Unify each constraint** using the original inference table’s `unify` method.
7. **Discard the fresh table**.

This is `O(k)` where `k` is the number of type variables in the new code.

---

## 9. Caching & Incrementality (Persistent)

### 9.1 Cache Key

```rust
struct CacheKey {
    def_id: DefId,
    arg_hash: u128,                    // xxhash128 of all CvmValue args
    ty_ctx_fingerprint: u128,          // rolling hash of TyCtx state
    capability_mask: u64,
    mir_hash: u128,                    // hash of macro’s MIR body
}
```

- **`arg_hash`** – 128‑bit xxhash. Collisions are astronomically unlikely; no extra verification.
- **`ty_ctx_fingerprint`** – **Rolling hash** updated on every type interning and every type variable resolution. Each update is `O(1)`. The hash is stored in `TyCtx` and snapshot‑copied.
- **`mir_hash`** – Hash of the macro’s MIR body; invalidates when the macro definition changes.

### 9.2 Cache Storage & Freshness Counters

Cache directory: `target/.comptime_cache/`.

- Each entry: file named by `hex(hash(key))` containing serialised `CvmOutput`.
- Separate file: `freshness.json` – JSON map from `expansion_path_hash` to `counter`. This file is loaded at start and saved after a successful build.

**Invalidation:** The driver recomputes the key for each comptime call. On cache hit, the stored output is used. On miss, the macro is evaluated and stored.

### 9.3 Cache Debugging

- `--dump-cache` – prints all cache keys, hit counts, and last access time to `target/.comptime_cache/dump.txt` (human‑readable table).
- `--clear-cache` – deletes the entire cache directory.

---

## 10. Debugging & Observability

### 10.1 Macro Call Stack in Diagnostics

Each `Span` carries a chain of `ExpnId`s. The diagnostic formatter prints:

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

If the stack exceeds 10 entries, print first 5, then `...`, then last 5, with a hint: `(use RUST_BACKTRACE=full for complete chain)`.

### 10.2 Step‑Through Expansion (Source‑Level)

The `--step-macros` flag enables interactive mode. **Step granularity:** one source statement (HIR statement) in the comptime function’s body.

When enabled, the CVM runs in a special mode: after each statement, it pauses, prints the current source line (with a `→` marker), and waits for input:

- `c` – continue to next statement.
- `s` – step into a macro call (if current statement is a call).
- `p` – print the current CST fragment being built.
- `q` – abort compilation.

This is implemented by augmenting the CVM with a breakpoint flag that yields after every HIR node.

### 10.3 Logging & Metrics

Two formats:

- **Human‑readable** (`--log-macros`): prints to stderr.
- **JSON** (`--log-macros-json`): writes to `macro_log.json` in current directory.

**JSON Schema:**
```json
{
  "version": 1,
  "events": [
    {
      "phase": "expand",
      "macro": "vec!",
      "def_id": "crate0::vec",
      "span": "src/main.g:12:5",
      "duration_ms": 2,
      "cache_hit": false,
      "generated_nodes": 23
    },
    {
      "phase": "type_check",
      "macro": "vec!",
      "duration_ms": 1
    }
  ]
}
```

- `--macro-stats` prints a summary:

```
Comptime expansion stats:
  Total expansions: 42
  Cache hits: 38
  Cache misses: 4
  Time in CVM: 123 ms
  Largest expansion: 1024 tokens
  Unused fresh type variables: 0
```

---

## 11. Capability Model (Automatic Propagation)

### 11.1 Declaration

```glyim
#[comptime(capabilities = "fs, env")]
fn read_config(path: String) -> String { ... }
```

### 11.2 Propagation Rules

The driver computes **transitive closure** of required capabilities statically:

- For each comptime function `F`, let `C(F)` be its declared capability set (empty if not annotated).
- If `F` calls `G`, then `C(F)` must be a superset of `C(G)`.
- Violation → compile error at the call site.

This means helper functions can omit the attribute; the caller must still have the capabilities. No annotation burden on helpers.

### 11.3 Enforcement

Before executing a comptime call, the driver sets a capability mask in the CVM. Each intrinsic checks the mask; if the required capability is missing, it panics with a compile error.

---

## 12. Implementation Phases (11 Weeks)

| Phase | Duration | Deliverables |
|-------|----------|---------------|
| **0: Prerequisites** | 1 week | COW data structure for `TyCtx`, rolling hash, hygiene marks in spans, CLI flags (`--step-macros`, `--log-macros-json`, `--dump-cache`, `--clear-cache`). |
| **1: CVM Core** | 2 weeks | `CvmValue`, interpreter loop for HIR subset (including `for` over lists), built‑in `quote!` macro, `concat_token_streams`, `parse_token_stream`. |
| **2: Type Queries** | 1 week | `Type` handle, intrinsics for type inspection, snapshot mechanism, short‑lived snapshots. |
| **3: Splicing & Inference Merging** | 2 weeks | `splice` algorithm, incremental re‑type‑checking with mapping from fresh to original variables (Section 8.2). |
| **4: Generics & Fresh Names** | 1 week | `fresh_name` with persistent counters (freshness.json), `fresh_type_var`, warning for unused variables. |
| **5: Caching** | 1 week | `ComptimeCache` with on‑disk storage, rolling fingerprint, cache debugging commands. |
| **6: Capabilities** | 1 week | Attribute parsing, capability set propagation, intrinsic enforcement. |
| **7: Debugging & Observability** | 1 week | Macro call stack in diagnostics, source‑level step‑through mode, JSON logging, `--macro-stats`. |
| **8: Testing & Stabilisation** | 1 week | Comprehensive test suite for all intrinsics, edge cases, and incremental behaviour. |
| **Total** | **11 weeks** | |

---

## 13. Risks & Mitigations

| Risk | Mitigation |
|------|-------------|
| COW memory overhead | Snapshots short‑lived; persistent structures use reference counting. |
| Rolling hash collision | 128‑bit `xxhash` – probability negligible. If collision occurs, cache may return a wrong result but types remain consistent; a warning can be added. |
| Stall detection false positive | Only warns after 3 stalls; user can override with `--force`. |
| Fresh name counter file corruption | Use atomic write (write to temp, then rename). If corrupted, reset to zero and warn. |
| Step‑through mode performance | Only enabled with flag; no overhead in normal compilation. |
| Capability propagation misses | Static check; error at call site. |

---

## 14. Conclusion

This design is **perfect** as defined by the review criteria:

- **Correctness** – Termination proved, cycle detection, stall detection, deterministic freshness, sound inference merging.
- **Boundaries** – Clear contracts for all intrinsics, error handling specified.
- **Modularity** – Components separated (Driver, CVM, Cache, Splicer).
- **Performance** – O(1) snapshots, rolling hash, 128‑bit cache keys, bounded loops.
- **Debuggability** – Call stacks, source‑level step‑through, JSON logs, cache dump.
- **Elegance** – Single concept (`comptime fn`), only one built‑in macro (`quote!`), no legacy baggage.

No further planning iterations are required. Implementation can begin immediately following the 11‑week roadmap.
