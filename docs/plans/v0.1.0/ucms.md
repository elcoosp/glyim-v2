# Ultra‑Parallel Implementation Plan for UCMS (10 Agents, ~3‑4 Weeks)

Based on the final design, we decompose the work into **50+ granular tasks** that can be distributed across **10 agents** working in parallel. The existing `glyim-codegen` crate (with `BytecodeBackend`) is **not reused** – the CVM is a new, simpler interpreter operating on a HIR subset. However, the `CodegenBackend` trait could be implemented for the CVM later if needed, but that’s out of scope.

## Crate Ownership Map

| Agent | Primary Crate(s) | Secondary Crate(s) |
|-------|------------------|--------------------|
| A1 | `glyim-span` | – |
| A2 | `glyim-syntax` | – |
| A3 | `glyim-cvm` (new) – Value & List ops | – |
| A4 | `glyim-cvm` – Intrinsics group 1 (type queries) | – |
| A5 | `glyim-cvm` – Intrinsics group 2 (diagnostics, I/O, freshness) | – |
| A6 | `glyim-frontend` – Fragment parser | – |
| A7 | `glyim-type` – Rolling hash, COW, counters | – |
| A8 | `glyim-solve` – Inference helpers | – |
| A9 | `glyim-typeck` – Incremental retype & merging | `glyim-pipeline` (splicer) |
| A10 | `glyim-cache` (new) – Cache & freshness store | `glyim-pipeline` (driver) |

---

## Phase 0: Foundation (Day 1‑2, 2 agents)

| Task | Agent | Deliverable | Dependencies |
|------|-------|-------------|--------------|
| `glyim-span`: add `ExpnId` chain to `Span` | A1 | `Span::with_chain()`, `push_expn()`, `call_stack()` formatter | None |
| `glyim-syntax`: define `TokenData` and `TokenStream` | A2 | `type TokenStream = Vec<(SyntaxKind, Arc<str>, Mark)>`; `concat()`, `from_str()` | A1 (Mark) |

**Done:** Both agents finish in parallel, merge at end of day 2.

---

## Phase 1: Core CVM (Day 2‑5, 1 agent)

| Task | Agent | Deliverable | Dependencies |
|------|-------|-------------|--------------|
| Create `glyim-cvm` crate | A3 | `Cargo.toml`, `lib.rs` | – |
| Implement `CvmValue` enum | A3 | All variants: `Int`, `Uint`, `Bool`, `String`, `Type`, `Lifetime`, `Expr`, `TokenStream`, `Span`, `List`, `Tuple` | A2 (TokenStream) |
| Implement `List<T>` operations | A3 | `len()`, `get()`, `push()`, `map()`, `fold()`, `for` loop support | – |
| Implement CVM interpreter core | A3 | Stack machine, evaluation of HIR subset (literals, `if`, `match`, `for` over lists, variable binding) | – |

**Agent A3** works alone for ~3 days.

---

## Phase 2: Type System Enhancements (Day 2‑4, 2 agents)

| Task | Agent | Deliverable | Dependencies |
|------|-------|-------------|--------------|
| `glyim-type`: add rolling hash field | A7 | `TyCtx` gains `fingerprint: u128`, updated on each type interning and type var creation | – |
| `glyim-type`: implement COW persistence | A7 | Use `im::HashMap` and `im::Vector`, add `snapshot()` that returns `TyCtxSnapshot` | – |
| `glyim-type`: add `total_type_vars_created()` | A7 | Counter increments on each `new_ty_var()`, `new_int_var()`, etc. | – |
| `glyim-solve`: add `unresolved_type_vars()` | A8 | Count inference variables with `value == None` | A7 (uses `TyCtx`) |
| `glyim-solve`: add `total_created()` | A8 | Return the same counter as A7 (or read from `TyCtx`) | A7 |

**A7 and A8 work in parallel** on different crates. A8 depends on A7’s API, but can start after A7 provides a minimal `TyCtx` with the counter (stubbed). Coordinate via weekly sync.

---

## Phase 3: Intrinsics – Type Queries (Day 3‑6, 1 agent)

| Task | Agent | Deliverable | Dependencies |
|------|-------|-------------|--------------|
| Implement `type_name` intrinsic | A4 | `extern "cvm" fn type_name(ty: Type) -> String` | A3 (CVM value passing), A7 (Type lookup) |
| Implement `type_fields` | A4 | Returns list of `(String, Type)` for structs, `(String, List<Type>)` for enums | A3, A7 |
| Implement `type_is_copy`, `type_is_sized` | A4 | Boolean queries | A7 |
| Implement `type_is_enum`, `type_variants` | A4 | Enum‑specific | A7 |
| Implement `type_generic_args` | A4 | Returns `List<CvmValue>` where each is `Type` or `Lifetime` | A3, A7 |

**Agent A4** works on these after A3 and A7 have basic APIs.

---

## Phase 4: Intrinsics – Diagnostics, I/O, Freshness (Day 4‑7, 1 agent)

| Task | Agent | Deliverable | Dependencies |
|------|-------|-------------|--------------|
| Implement `emit_diagnostic` | A5 | Forwards to `DiagSink`, levels 0‑3 | A3 |
| Implement `compile_error` | A5 | Aborts with error | A3 |
| Implement `parse_token_stream` | A5 | Uses `glyim-frontend` (stubbed until A6 done) – returns `Result<TokenStream, String>` with offset | A6 (later) |
| Implement `read_file` | A5 | Uses `std::fs`, capability‑checked | A3 |
| Implement `get_env_var` | A5 | Uses `std::env`, capability‑checked | A3 |
| Implement `fresh_name` | A5 | Uses `FreshnessStore` (A10) – returns string | A10 (later) |
| Implement `fresh_type_var` | A5 | Creates new inference var via `TyCtx` | A7 |

**Agent A5** can start early with stubs for dependencies; final integration after A6 and A10.

---

## Phase 5: Fragment Parser & Splicing (Day 3‑6, 2 agents)

| Task | Agent | Deliverable | Dependencies |
|------|-------|-------------|--------------|
| `glyim-frontend`: add `parse_token_stream_fragment()` | A6 | Parses a `TokenStream` into a `SyntaxNode` (CST fragment) | A2 |
| `glyim-pipeline` (splicer module): implement `splice()` | A9 | Replaces macro call node with fragment, records range, returns new CST | A6 |

**A6 and A9** can work in parallel: A9 writes the splicing logic assuming a `parse_fragment` function exists (stub). They integrate after both are ready.

---

## Phase 6: Incremental Re‑type‑checking (Day 5‑8, 1 agent)

| Task | Agent | Deliverable | Dependencies |
|------|-------|-------------|--------------|
| `glyim-typeck`: implement `retype_new_nodes()` | A9 | Fresh inference table, map variables, emit equality constraints, unify with original | A7, A8, A6, A3 (value mapping) |

**Agent A9** (already working on splicing) continues into this task. Can be done in parallel with caching (A10).

---

## Phase 7: Caching & Freshness Store (Day 4‑8, 2 agents)

| Task | Agent | Deliverable | Dependencies |
|------|-------|-------------|--------------|
| Create `glyim-cache` crate | A10 | `Cargo.toml`, `lib.rs` | – |
| Implement `ComptimeCache` | A10 | `CacheKey` struct, `xxhash128`, persistent storage, `--dump-cache`, `--clear-cache` | A7 (fingerprint), A2 (TokenStream serde) |
| Implement `FreshnessStore` | A10 | `freshness.json` load/save, atomic write, per‑expansion path counters | A1 (ExpnId chain) |
| Integrate cache into `ExpansionDriver` | A10 | Driver uses cache for lookups | A10, A3 (output serialisation) |

**Agent A10** can work on caching without waiting for the driver; the driver (A1 later) will integrate.

---

## Phase 8: Expansion Driver & Pipeline Integration (Day 6‑10, 1 agent)

| Task | Agent | Deliverable | Dependencies |
|------|-------|-------------|--------------|
| `glyim-pipeline`: create `ExpansionDriver` | A1 | Worklist, state machine, cycle detection, measure `(M1, M2)`, stall detection | A7, A8, A9, A10, A3, A5 |
| Integrate driver into main `compile_file()` | A1 | Replace old macro expansion, call `expand_until_fixed_point()` | – |

**Agent A1** (who did `glyim-span` earlier) now builds the driver, using all other components.

---

## Phase 9: Capabilities & Sandboxing (Day 7‑9, 1 agent)

| Task | Agent | Deliverable | Dependencies |
|------|-------|-------------|--------------|
| Attribute parsing for `#[comptime(capabilities = ...)]` | A7? | Extend `glyim-hir` to store `capabilities: CapabilitySet` | – |
| Capability propagation (transitive closure) | A7 | Driver computes `C(F)` superset check | A1 (call graph) |
| Enforce in CVM intrinsics | A5 | Each intrinsic checks a global mask | A5 (already in intrinsics) |

**Agent A7** (who did type enhancements) can switch to capabilities after finishing type work. Or assign a new agent (A11) if available – we have only 10, so A7 takes this.

---

## Phase 10: Debugging & Observability (Day 8‑10, 2 agents)

| Task | Agent | Deliverable | Dependencies |
|------|-------|-------------|--------------|
| Step‑through mode (`--step-macros`) | A1 (or new) | Interactive REPL, source‑level breakpoints | A1 (driver) |
| JSON logging (`--log-macros-json`) | A1 | Log events, schema | A1 |
| `--macro-stats` summary | A1 | Print metrics | A1 |

**These can be done by A1 (driver author) or by splitting to A11 (we have only 10, so A1 continues).** To keep parallelism, we can give this to A9 after they finish retype merging.

---

## Phase 11: Testing & Stabilisation (Day 9‑12, 2 agents)

| Task | Agent | Deliverable | Dependencies |
|------|-------|-------------|--------------|
| Unit tests for CVM intrinsics | A3 + A4 + A5 | Test each intrinsic with edge cases | All intrinsics |
| Integration tests for splicing | A6 + A9 | `quote!` macros, code generation | A6, A9 |
| Incremental cache tests | A10 | Persistence, fingerprinting, `--dump-cache` | A10 |
| End‑to‑end test suite | All | Run all `run-pass` and `compile-fail` tests | Everything |

**Two agents** (e.g., A3 and A10) can coordinate testing.

---

## Gantt Chart (10 agents, 12 days)

```
Day:   1 2 3 4 5 6 7 8 9 10 11 12
A1:    ██ [span] ░░░░░░░████████ [driver] ██████ [debug]
A2:    ██ [syntax] ░░░░░░░░░░░░░░░░░░░░░░░░░░
A3:    ░░████████ [CVM core] ░░░████ [intrinsics?] ██ [tests]
A4:    ░░░░████████ [type queries] ░░░░░░██ [tests]
A5:    ░░░░░░████████ [diag/io/fresh] ░░██ [tests]
A6:    ░░░░████ [fragment parser] ░░░░░░░░██ [tests]
A7:    ░░██████ [type rolling+COW] ░██ [caps] ░██ [tests]
A8:    ░░████ [inference helpers] ░░░░░░░░██ [tests]
A9:    ░░░░░░██████ [splicing+retype] ░██ [tests+debug?]
A10:   ░░░░░░████████ [cache+freshness] ░██ [tests]
```

**Legend:** █ = work, ░ = idle/integration, numbers = day.

---

## Integration Points (Critical Merge Points)

- **End of Day 2:** A1 (span) + A2 (syntax) merged → base for token streams and hygiene.
- **End of Day 4:** A7 (type rolling) + A8 (inference helpers) → type context ready for queries.
- **End of Day 5:** A3 (CVM core) + A4 (type queries) → first working `type_name`.
- **End of Day 6:** A6 (fragment parser) + A9 (splicing) → code generation works.
- **End of Day 7:** A5 (fresh name + I/O) + A10 (freshness store) → `fresh_name` deterministic.
- **End of Day 8:** A9 (retype merging) + A1 (driver) → fixed‑point loop operational.
- **End of Day 10:** A1 (debug features) + A5 (capabilities) → complete system.
- **Day 11‑12:** Testing & stabilisation.

---

## Total Parallelism Achieved

- **Peak active agents:** 10 (all working simultaneously from day 3 to day 8).
- **Total calendar time:** 12 days (2.5 weeks) – down from 11 weeks (87% reduction).
- **Actual elapsed time with 10 agents:** ~3 weeks including final stabilisation.

**This plan is ready to execute.** Each agent has a clear crate ownership and task list. The dependencies are minimal, and all integration points are planned.
