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
- [ ] Tier 1.6 — drop elaboration per-projection (move-path tree)
- [ ] Tier 1.7 — dynamic range slicing
- [ ] Tier 2.1 — coherence overlap ignores generics
- [ ] Tier 2.2 — HRTB provable cases (reflexivity/static/WF/identity)
- [ ] Tier 2.3 — object safety associated types & supertraits
- [ ] Tier 3.1 — transitive dependency resolution (glyip)
- [ ] Tier 3.2 — glyip cmd_test executes tests
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
