# Stub Remediation — Work Changelog

Plan: `docs/plans/v0.1.0/unstub-final/`
Each tier is implemented and committed atomically. Tests are run with
`cargo nextest run -p <crate>` for affected crates.

## Status

- [x] Tier 0 — mir-interp soundness (0.1 sizing, 0.2 ConstantIndex/Subslice write, 0.3 PtrToPtr comment, 0.4 Drop/Call scope note, 0.5 Len non-int -> Err) — COMMITTED
- [x] Tier 1.1 + 1.1b — closure capture analysis + real closure ADT type — COMMITTED
- [x] Tier 1.2 — vtable generation (trait-def-driven method population) — COMMITTED
- [x] Tier 1.3 — Iterator::next real resolution — COMMITTED
- [x] Tier 1.4 — Range lowering bug — COMMITTED
- [x] Tier 1.5 — const-eval expression coverage (Return/Loop/While/Flow) — COMMITTED
- [x] Tier 1.6 — drop elaboration per-projection (move-path tree) — COMMITTED (top-level/whole-value)
- [x] Tier 1.7 — dynamic range slicing — COMMITTED (regression coverage; impl already present)
- [x] Tier 2.1 — coherence overlap ignores generics — COMMITTED
- [x] Tier 2.2 — HRTB provable cases (reflexivity/static/WF/identity) — COMMITTED
- [x] Tier 2.3 — object safety associated types & supertraits — COMMITTED
- [x] Tier 3.1 — transitive dependency resolution (glyip) — COMMITTED
- [x] Tier 3.2 — glyip cmd_test executes tests — COMMITTED
- [ ] Tier 3.3 — registry-disabled error message
- [ ] Tier 4.1 — fragment-spec matching (Stage A + B)
- [ ] Tier 4.2 — line!/column! from SourceMap
- [ ] Tier 4.3 — include! CWD-relative fix
- [ ] Tier 4.4 — stringify! normalization
- [ ] Tier 5.1 — over-alignment fallback comment + set_alignment
- [ ] Tier 5.2 — DWARF pointer/slice debug types
- [ ] Tier 5.3 — fn_sig fallback -> hard error
- [ ] Tier 5.4 — bytecode backend Subslice/ConstantIndex scaling
- [ ] Tier 6.1-6.5 — LSP reference graph/rename/completion/unused-imports
- [ ] Tier 7.1-7.4 — test harness real linking+execution+mock wiring

## Commits

### Tier 0 (mir-interp)
- `fix(interp): real element sizing for pointer arithmetic` — `get_element_size`
  now calls `SimpleLayoutComputer`. `ConstantIndex`/`Subslice` write paths
  implemented. `PtrToPtr` no-op documented; `Drop` scope documented + panic
  unwind flag. Test `tier0.rs` added (3 tests, 178 crate tests pass).

### Tier 1.1 + 1.1b (typeck closures)
- `feat(typeck): real closure capture analysis + closure ADT type`
  - `LocalEnv::next_var_id()` + capture boundary filtering by `LocalVarId`.
  - `capture_log` on `FnCtxt`; `check_path` records VarRefs; mut-use flagged
    in `Expr::Assign` lhs and `Expr::Ref { Mut }`.
  - Closure arm checks body once inside its scope (cache cleared first so the
    body re-resolves as a capture), classifies ByRef(Not)/ByRef(Mut).
  - `TyCtxMut::register_closure`/`next_synthetic_adt_id` build a real closure
    ADT (seeded at id 2_000_000 to avoid colliding with builtins/user ADTs).
  - Test `closures.rs` asserts 1 capture of enclosing `x` as ByRef(Not) and
    that the closure type is a concrete `Adt` (not Infer).
  - 56 typeck tests pass.

### Tier 1.2 (layout / vtable generation)
- `feat(layout): derive vtable method slots from the trait definition`
  - Plan deviation (documented): the codebase has no `ImplDef.items` registry
    and `glyim-solve::TraitContext::ImplDef` carries no method bindings. The
    real trait-method data lives in `glyim_type::TraitDef { methods: Vec<MethodDef> }`
    on `TyCtx`. Added `TyCtxMut::register_trait_def(id, TraitDef)` to populate
    it (mirrors `register_adt`/`register_fn_sig`).
  - `MethodDef` gained `fn_def_id: Option<FnDefId>` so each vtable entry can
    carry the concrete dispatch target.
  - `SimpleLayoutComputer::vtable_of` now looks up `trait_def(id)` and builds
    one `VTableEntry` per trait method (name + sig + fn_def_id), instead of
    the previous `methods: vec![]` placeholder. Traits with no registered
    method set still produce an empty vtable (backward compatible).
  - New test `s15_vtable_computer_populates_methods_from_trait_def` registers a
    2-method trait and asserts the vtable carries both entries with correct
    def ids and size. 63 glyim-layout + 168 glyim-codegen tests pass;
    `cargo check --workspace` clean.

### Tier 1.3 (Iterator::next resolution)
- `fix(solve): Iterator::next yields a real Option<T> type`
  - Plan deviation (documented): `iterator_next_info` already returned a real
    `SolverIteratorNextInfo` (not `None`) — it derived `next` from the
    registered `builtin_next_fn_id`. The genuine latent bug was the `Option`
    return type using a magic `AdtId::from_raw(101)` with no backing ADT.
  - Registered a real `Option<T>` builtin enum (id 1010, variants `None`/
    `Some(T)`) in `TyCtxMut::register_builtin_ranges`, and pointed
    `iterator_next_info` at it. Avoided id collision with existing tests that
    use 1006/1007.
  - Tests: `test_option_builtin_registered` (glyim-type) asserts the enum is
    registered with two correct variants; `t18_iterator_next_info_uses_real_
    option_adt` (glyim-solve) asserts `None` when no `next` fn is registered
    and a real `Option<elem>` (id 1010) otherwise. 560 glyim-type+glyim-solve
    tests pass; `cargo check --workspace` clean.

### Tier 1.4 (Range lowering)
- `fix(lower): lower ranges to real Range/RangeInclusive aggregates`
  - The `thir::ExprKind::Range` arm in `lower_expr_to_rvalue` discarded
    start/end/inclusive and returned an empty `Aggregate(Tuple, [])` (the bug).
  - Typeck now resolves the range expression's type to the proper
    `Range<T>` (id 1000, exclusive) / `RangeInclusive<T>` (id 1001, inclusive)
    ADT via `mk_adt`, deriving `T` from the endpoint types (defaulting to the
    error type for a full `..`).
  - Lowering reads the substitution from that resolved `Adt` type and emits
    `Aggregate(Adt(adt_id, 0, substs), [start_op, end_op])`, so the range
    carries its real endpoints. `..` (no endpoints) emits Error-typed operands.
  - Tests `range_lower.rs` (3 tests) assert exclusive -> Adt 1000, inclusive ->
    Adt 1001, both with 2 constant operands. 187 glyim-lower + 56 glyim-typeck
    tests pass; `cargo check --workspace` clean.

### Tier 1.5 (const-eval expression coverage)
- `feat(const-eval): support Return/Break/Continue and while/loop`
  - `Expr::Return { value }` now evaluates the inner expression (or Unit) and
    returns it instead of erroring.
  - Added a `loop_control: Option<LoopControl>` field + `LoopControl::{Break,
    Continue}` enum. `Break`/`Continue` set the flag (value currently ignored,
    matching unlabeled break in const eval) instead of erroring.
  - New `eval_while` / `eval_loop` drivers constant-fold loops: evaluate the
    condition/body, react to `break` (exit -> Unit) and `continue` (next
    iteration). Both are bounded by a 1_000_000-iteration cap so a
    non-terminating loop returns a clear error instead of hanging.
  - Note: `Call`/`MethodCall`/`Closure`/`Range`/`For` remain unsupported
    (genuinely require a full evaluation context); `For` over a constant
    iterable was left as a future extension.
  - Tests `flow_eval.rs` (6) cover Return-with/without value, while-false (body
    not run), while-true-with-break, loop-with-break, and continue re-entry via
    an if/break/continue body. 67 glyim-const-eval tests pass; `cargo check
    --workspace` clean.

### Tier 1.6 (drop elaboration — top-level / whole-value)
- `feat(lower): elaborate scope drop terminators for non-Copy locals`
  - Plan deviation (documented): the codebase has no `drops.rs` /
    `elaborate_drops` module; the lower pass never emitted any `Drop` or
    `StorageDead` terminator at all — values going out of scope simply leaked
    (only `TerminatorKind::Drop` fed into `MonoItem::DropGlue` in mono).
  - `MirBuilder::lower_body` now calls `elaborate_scope_drops` on the
    fall-through block (the same condition the old `terminate(Return)` used:
    `current_block.is_some()`). It inserts a chain of `Drop { place, target,
    cleanup: None }` terminators in **reverse declaration order** for every
    local whose type needs a destructor, routing the last drop into a fresh
    `Return` block. The return place (`_0`) and parameters are skipped.
  - Added `needs_drop(ty)`: `Copy` types never drop; ADTs/composites recurse
    into the registered `LowerCtx::adt_def` fields; `String` (and other owning
    types) need drop; references/raw pointers/closures/fn-ptrs/dyn/projections/
    inference/params are treated as no-drop to avoid spurious destructor calls.
    Because the test `TestLowerCtx`/`LocalMockLowerCtx` return empty ADT fields,
    struct locals in existing tests stay no-drop and their `block_count`
    assertions are preserved.
  - Correctness guard: the drop chain is only injected when control **falls
    straight through** to the current block. For `if`/`match`/`while`/`loop`
    the lowering already redirected control flow and set `current_block =
    None`, so their real terminators (SwitchInt/Goto/Call/Return) are left
    untouched — fixing an early bug where injecting drops on `entry`
    clobbered those terminators.
  - Tests `drop_elaboration.rs` (3): a `String` local gets a `Drop` terminator
    (entry routes through the chain, not directly to Return); an `i32` local
    gets no drop (single Return block); a function returning `String` does not
    drop `_0`. 190 glyim-lower tests pass; `cargo build --workspace` clean.

### Tier 1.7 (dynamic range slicing — `arr[i..j]`)
- `test(lower): lock in dynamic range slicing lowers to a {ptr, len} tuple`
  - Plan deviation (documented): `lower_dynamic_range_slice` was **already
    implemented** in `glyim-lower/src/lower_rvalue.rs` (≈160 lines). It builds
    the slice as an ordinary `Rvalue` — `Len(base)` for length, runtime
    `start`/`end` operand evaluation (defaulting start=0 / end=len), two
    bounds-check `SwitchInt` asserts (start<=end, end<=len), then
    `data_ptr = first_elem + start*elem_size` and `new_len = end - start`,
    returning `Aggregate(AggregateKind::Tuple, [ptr, len])`. The standalone
    `thir::ExprKind::Range` arm already routes a `Range` index into it.
  - THIR limitation surfaced: `thir::Range` bounds are `Option<Box<Expr>>`
    carrying only literal expressions (no runtime-local bounds), so a fully
    dynamic `arr[i..j]` with `i`/`j` as function parameters cannot yet be
    expressed at the THIR level. The lowering nonetheless emits runtime
    arithmetic + bounds checks (not a `Place` projection), matching the
    `slice_desugar.rs` design note that this belongs in `glyim-lower`.
  - Added regression tests `dynamic_range_slice.rs` (3): `arr[1..4]` lowers to
    a `{ptr, len}` tuple with no error diagnostics; `arr[1..=4]` (inclusive) is
    correctly rejected with a diagnostic; `arr[1..]` (open-ended) lowers to a
    `{ptr, len}` tuple. 193 glyim-lower tests pass; `cargo build --workspace`
    clean.

### Tier 2.1 (coherence overlap check ignores generic args)
- `fix(typeck): structural_tys_match recurses into Adt substitution args`
  - In `glyim-typeck/src/coherence.rs`, the `structural_tys_match` `Adt` arm
    previously discarded the `Substitution` (`id_a == id_b`) so `Vec<i32>` and
    `Vec<String>` were wrongly treated as overlapping — a soundness gap that
    would reject valid non-overlapping impls. The arm now recurses into
    `ctx.substitution_args`, comparing each `GenericArg::Ty` structurally;
    lifetime/const args are treated as always-compatible (documented, since
    const values aren't modeled precisely yet). The `Tuple`/`Array`/`Slice`/
    `Ref`/`RawPtr` arms already recursed into inner types.
  - Test-harness fix (latent bug): `type_ref_to_ty` resolved primitive type
    names via `ctx.name_str(name)`, but `ctx`'s interner is a *separate*
    `Interner` instance from the one that interned the test names
    (`Interner` wraps `Arc<ThreadedRodeo>`, each `new()` is its own symbol
    table). That returned an empty/garbage string and silently produced
    `Ty::ERROR` for generic args — which is why no prior test caught it (they
    resolve via `def_map`, and blanket-vs-concrete short-circuits on the param
    branch). Switched to `interner.resolve(name)` (the interner that created
    the names).
  - Added `make_generic_impl_item` + tests `coherence.rs` `t11` (distinct
    generic self types `Vec<i32>`/`Vec<String>` do NOT conflict) and `t12`
    (identical `Vec<i32>`/`Vec<i32>` DO conflict). 12 coherence / 58 glyim-
    typeck tests pass; `cargo build --workspace` clean.

### Tier 2.2 (HRTB predicates — mostly `Ambiguous` → `Proven` for trivial cases)
- `fix(solve): cheap HRTB provable-case wins in check_hrtb`
  - `glyim-solve/src/hrtb.rs` `check_hrtb` now resolves the *trivially
    provable* HRTB predicates to `SolverResult::Proven` instead of the prior
    blanket `Ambiguous`:
    - `RegionOutlives(a, b)`: reflexivity (`a == b`) and any side being
      `'static` → `Proven`. Two *distinct* placeholders (or a placeholder vs an
      unrelated early-bound region) correctly stay `Ambiguous` (not falsely
      proven).
    - `TypeOutlives(ty, r)`: `r == 'static` → `Proven`; if `r` is structurally
      *inside* `ty` (reflexive containment, via new `region_in_ty`) → `Proven`;
      otherwise an owned/scalar `ty` with no open components (`ty_has_open_
      components`) → `Proven`; genuinely open types stay `Ambiguous`.
    - `WellFormed(ty)`: concrete types with no open components
      (inference vars, generic params, late-bound placeholders, region
      variables) and not `dyn`/projection → `Proven`; else `Ambiguous`. Helper
      `ty_is_concrete_well_formed` deliberately excludes `HAS_RE_PLACEHOLDER`
      (a placeholder region introduced by HRTB instantiation is fully
      resolved, not "open").
    - `Coerce(a, b)`: identity coercion resolved via a new structural
      `ty_struct_eq` (recurses through substitution args), then falls back to
      `can_coerce`.
  - Root cause surfaced: `Ty`/`Substitution`/`Region` equality is by *interned
    handle*, but HRTB instantiation **re-interns** substituted types, so two
    structurally-identical types (e.g. `fn(&'p i32)`) carry different handles
    and the old index-based `a == b` identity check missed them. `ty_struct_eq`
    compares structurally (and through substitution contents), fixing both the
    `Coerce` identity case and making identity resolution robust.
  - Wired the previously-orphaned `tests/hrtb.rs` into `tests/mod.rs` (it was
    not declared, so none of its `test_hrtb_*` tests ran). Updated the
    pre-existing `test_hrtb_coerce_via_check` expectation from `Ambiguous` to
    `Proven` (identity `Coerce(i32,i32)` is now correctly proven).
  - Added 6 regression tests `t22_*`: region-outlives reflexivity (Proven),
    distinct-placeholder region-outlives (Ambiguous), type-outlives reflexive
    `&'a i32: 'a` (Proven), type-outlives owned `i32: 'a` (Proven),
    well-formed `&'a i32` (Proven), identity `Coerce(fn(&'a i32), fn(&'a i32))`
    (Proven). 33 glyim-solve lib tests pass; `cargo build --workspace` clean.
  - DOC NOTE left on `check_hrtb` stating which cases remain conservative and
    why (this is the 80% cheap-win pass, not a complete HRTB solver).

### Tier 2.3 (object safety ignores associated types & supertraits)
- `feat(type): object safety checks associated types + supertraits`
  - `glyim-type/src/object_safety.rs` `check_object_safety` previously only
    inspected per-method receiver/generic-param shape. Replaced its
    `(requires_self_sized, methods)` signature with a `TraitObjectSafetyInput`
    struct (associated types + supertraits are trait-level, not method-level,
    so they are NOT bolted onto `MethodSignature`).
  - Added `AssociatedTypeInfo { name, span, is_constrained_in_all_methods }`
    and `SupertraitSafety { trait_id, is_safe, span }`. New violation variant
    `ObjectSafetyViolation::SupertraitNotObjectSafe { trait_id, span }`
    (cleanly named rather than overloading `SelfSized`). The pre-existing-but-
    never-constructed `UnconstrainedAssociatedType { name, span }` variant is
    now actually emitted.
  - Two new check folds in `check_object_safety`: an associated type that is
    not constrained (mentioned) by the methods yields
    `UnconstrainedAssociatedType`; a supertrait whose pre-resolved
    `is_safe == false` yields `SupertraitNotObjectSafe`. `glyim-type` stays
    pure/data-in-data-out — the caller (`glyim-typeck`, which owns the
    recursive `TraitDef.predicates` walk) computes `supertrait_safety` and
    passes it in, exactly as the plan's design note requires.
  - Updated the only two call sites (both test-only): `glyim-type/src/tests/
    s12_object_safety.rs` and `glyim-codegen/src/tests/trait_objects.rs`
    (8 call sites there) now build `TraitObjectSafetyInput`. Added 4 new
    regression tests in `s12_object_safety.rs`: unconstrained associated type
    is flagged, constrained associated type is NOT, unsafe supertrait is
    flagged, safe supertrait is NOT. 13 object-safety / 557 glyim-type tests
    pass; 4 object-safety / 168 glyim-codegen tests pass; `cargo build
    --workspace` clean.

### Tier 3.1 (transitive dependency resolution in glyip)
- `feat(glyip): resolve transitive dependencies (BFS enqueues sub-deps)`
  - `crates/glyip/src/dep.rs`: `DependencyResolver::resolve` previously seeded
    the BFS queue only from the root's direct deps and never enqueued a
    resolved crate's own dependencies (the `// TODO: implement transitive
    resolution` was duplicated twice — both removed).
  - `IndexEntry` gained a `dependencies: HashMap<String, Vec<IndexDependency>>`
    field (`#[serde(default)]` for backward-compat with on-disk `.json`
    index files) plus a new `IndexDependency { name, version_req }` struct.
  - `resolve_registry_dep` and `resolve_path_dep` now return
    `(LockedCrate, Vec<(String, Option<String>, Option<PathBuf>)>)` — the sub-
    dep list — instead of just `LockedCrate`. Their `LockedCrate.dependencies`
    (a `BTreeMap<String,String>`) is now populated from that list (the
    requested version requirement), not left `BTreeMap::new()`. A shared
    `sub_deps_from_index` helper reads `entry.dependencies[version]`.
  - `resolve()`'s loop now enqueues each resolved crate's sub-deps. The queue
    element was extended with a 4th field `Option<PathBuf>` "base dir": a path
    sub-dependency carries its relative path and its nested path deps must
    resolve against that crate's directory, so the enqueued base is set to the
    parent's abs dir; registry sub-deps carry `None`/`None`. Diamond deps are
    still handled by the existing `visited` set keyed on `{name}-{version}`
    (Cargo-style non-unification); semver-range unification left out of scope
    per the plan.
  - Existing `IndexEntry { ... }` literals in the three test files
    (`crate_index_io.rs`, `dep_advanced.rs`, `dep_resolution.rs`, ~22 sites)
    were updated to include `dependencies: Default::default()` (and
    `IndexDependency` imported where the new test uses it).
  - Added regression test `resolve_transitive_dependencies`: a local index has
    `a` depends-on `b`; root depends only on `a`; asserts `b` appears in the
    lockfile and `a.dependencies["b"] == "0.5"`. 177 glyip tests pass;
    `cargo build --workspace` clean.

### Tier 3.2 (glyip `cmd_test` actually executes tests)
- **Adaptation (plan assumed native link+run; reality differs):** the bytecode
  backend emits glyim's own opcode format (see `glyim-codegen`
  `BytecodeBackend::generate`), NOT a native object file, so the plan's
  "link + run the bytecode object" path is non-functional on this tree. The
  only in-repo working runtime is the MIR interpreter (`glyim_mir_interp`),
  which `glyim-test`'s `InterpRunner` already uses. `cmd_test` now compiles each
  discovered test file to MIR and runs the specific test body via
  `Interpreter::run_body`, which is genuine execution, not a stub.
- Added `glyim_pipeline::compile_file_to_mir` (returns `MirCompilation {
  bodies: HashMap<DefId, Arc<Body>>, def_map: CrateDefMap, ty_ctx: Arc<TyCtx>
  }`) — the front half of `compile_file` without backend codegen.
- `cmd_test` (commands.rs): replaced the file-counting stub with per-test
  compile-to-MIR + run. Resolves each discovered test name to its `DefId` via
  the `CrateDefMap` (`resolve_test_def_id`), registers all bodies in the
  interpreter, and runs the matching body — `Ok` => passed, `Err` => failed.
  Compilation failures also count as failed. `#[ignore]` tests are skipped
  unless `TestOptions::run_ignored` is set (mirrors `cargo test -- --ignored`).
- `test_discovery::DiscoveredTest` gained an `ignored: bool` field. Because
  glyim source does NOT parse `#[...]` attributes (the frontend lexer emits
  `Hash` but the parser forms no attribute node — `scan` of a `#[ignore]`
  attribute yields "expected item, found Hash"), the `#[ignore]` marker is
  written as a comment (`// #[ignore]`) on the line before the function; `scan`
  detects both the bare `#[ignore]` attribute and a `//`-comment containing
  "ignore".
- `TestOptions` gained `run_ignored: bool` (already derived `Default`).
- **Known pipeline quirk (documented, not fixed here):** the MIR pipeline
  drops the LAST-declared function in a file from `thir_bodies`, so a test that
  is the final function in its file will not get a runnable body. Test fixtures
  work around this with a trailing `_pad() {}` non-test function. (Fixing the
  underlying typeck/THIR collection is a separate task.)
- Tests: `test_cmd_test_executes_tests` (1 passed / 1 failed / 1 ignored via
  two files), `test_cmd_test_runs_ignored_when_requested` (both run when
  `run_ignored`), plus updated `test_with_only_src_files` / `test_with_filter`
  to the new discovered-test-function semantics. 179 tests pass; `cargo build
  --workspace` clean; `glyim-pipeline` (3) and `glyim-test` (94) still green.
