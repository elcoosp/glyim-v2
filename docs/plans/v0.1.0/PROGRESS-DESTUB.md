# GLYIM_DESTUB_PLAN — Progress Tracker

**Standing goal:** implement everything in `docs/plans/v0.1.0/GLYIM_DESTUB_PLAN.md` with no gaps, no input until green. Always use `cargo nextest`. Commit atomically per file, then push.

## Status (updated 2026-08-25)

- **Baseline suite:** 4116 tests, all green via `cargo nextest` (the interner + full pipeline must stay green).
- **Phase 1 (Iterator for-loop fallback) — DONE & GREEN.**
  - Two real blockers were fixed (NOT the root cause the plan guessed):
    1. Typeck `Expr::Block` called `check_expr` on statement nodes (`let`/`assign`/`return`), which hit broken arms returning `Err`. Fixed by adding `FnCtxt::check_stmt_to_thir` (in `check_stmt.rs`) and routing both the top-level loop and `Expr::Block` through it.
    2. Interpreter enum values dropped the variant discriminant: `Rvalue::Aggregate` for an enum ADT now prepends the discriminant tag (`fields[0]`); `read_place`'s `Downcast` strips it so payload fields stay at natural indices; `SwitchInt`/`Rvalue::Discriminant` read `fields[0]` = discriminant.
  - Test `for_loop_iterates_multiple_times_via_pipeline` passes (sum 10 via real iterator, not 0 via fallback).
  - All 198 `glyim-mir-interp` tests + 4116 workspace tests pass.
- **Phase 2 (P0) Const-generic array drop glue — DONE & GREEN.**
  - Step 2a: `TyCtx::subst_ty` (`ty_ctx_mut.rs`) now has a `TyKind::Array` arm that substitutes both the element type AND a `ConstKind::Param` length (using a unified `HashMap<u32, GenericArg>` substitution map; const args flow from `mono.rs`'s `GenericArg::Const` handling).
  - Step 2b: `mark_used_params` (`polymorphize.rs`) now also tracks the array length const's param index via `mark_used_params_in_const`, so polymorphize won't merge monomorphizations that differ only in array length.
  - Step 2c: `generate_array_drop_glue` (`mono_cache.rs`) now **panics** on a non-monomorphic length instead of silently emitting a bare `Return` (which previously skipped every element destructor). It emits one `Drop` terminator per element in forward order.
  - Tests: `subst_ty_substitutes_array_element`, `subst_ty_substitutes_const_param_array_length` (type unit tests); `drop_glue_for_array_of_droppable_drops_each_element`, `const_generic_array_drop_glue_resolves_length_through_substitution`, `const_generic_array_drop_glue_panics_on_unresolved_length` (pipeline unit tests). All pass. Full 4116 suite green.
- **Phase 4 (P1) Partial moves / drop-flag population — DONE & GREEN.**
  - All of Steps 4a–4c were already implemented (prior session): field `Field` arm in `lower_rvalue.rs` uses `is_copy` to choose `Move` vs `Copy` and calls `register_partial_move` for non-Copy fields; `register_drop_flag_init` pre-allocates + initializes a per-local drop-flag to `true` at declaration; `register_partial_move` clears it to `false` at the move site; `elaborate_scope_drops` (`builder.rs`) guards each `Drop` behind a `SwitchInt` on the flag so a partially-moved parent's destructor is skipped (sound: never double-frees; may over-retain sibling fields — noted as v2).
  - Honest MIR-level regression test added: `droppable_local_gets_guarded_drop_via_drop_flag` asserts a `SwitchInt` drop-flag guard (reading a flag local distinct from the dropped local) is emitted for a `String` binding. The plan's String-counting end-to-end test is infeasible here: the interpreter does not invoke user `Drop` impls (same gap as Phase 2), so runtime drop-counting cannot observe behavior. MIR-level guard verification is the achievable proof.

## Next phases (from plan, in priority order)
- Phase 5 (P1) — Deref autoderef for ADTs: `TyCtx::deref_ty` only handles `Ref`/`RawPtr`.
- Phase 3 (P1) — Async multi-poll returns `Pending` forever: `glyim-hir/src/lower/lower_async.rs` ~L814-1051.
- Phase 6 (fix) — `ConstValue::Range` falls back to zero-init in `cv_const`.
- Phase 9 (P3) — Inclusive range slicing (`..=`) in `lower_rvalue.rs`.
- Phase 7 (P2) — Windows SEH uses Itanium landingpads (toolchain gap; validate on real MSVC target).
- Phase 8 (P2) — Proc-macro two-stage build: only `glyim-cli` orchestration missing (cdylib/loader/Registry already done).

## Safety rule (plan §P3): never let unsupported-shape detection silently fall through to old skeleton behavior.

## Restart note
- Verify compiler behavior via REAL `PipelineCompiler` path, not standalone `glyim-cli`.
- Do NOT force-enable intentionally-disabled `harness_tests` module.
- Keep individual tool-call arguments under ~8K tokens.
- Plan line numbers are STALE — locate symbols by NAME.
- NEVER include API keys/tokens/passwords/credentials; redact as `[REDACTED]`.
