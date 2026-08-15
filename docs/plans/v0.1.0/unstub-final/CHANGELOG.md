# Stub Remediation — Work Changelog

Plan: `docs/plans/v0.1.0/unstub-final/`
Each tier is implemented and committed atomically. Tests are run with
`cargo nextest run -p <crate>` for affected crates.

## Status

- [x] Tier 0 — mir-interp soundness (0.1 sizing, 0.2 ConstantIndex/Subslice write, 0.3 PtrToPtr comment, 0.4 Drop/Call scope note, 0.5 Len non-int -> Err) — COMMITTED
- [x] Tier 1.1 + 1.1b — closure capture analysis + real closure ADT type — COMMITTED
- [ ] Tier 1.2 — vtable generation (ImplDef.items + VTableComputer + pipeline)
- [ ] Tier 1.3 — Iterator::next real resolution
- [ ] Tier 1.4 — Range lowering bug
- [ ] Tier 1.5 — const-eval expression coverage (Loop/While/Flow/Call/etc)
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
