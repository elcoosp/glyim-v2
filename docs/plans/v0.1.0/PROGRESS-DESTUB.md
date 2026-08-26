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
- **Phase 3 (P1) Async multi-poll — PARTIAL (v1 wired + safety net), M4/M5 deferred.**
  - M0–M3 SHIPPED & GREEN (prior sessions): `compute_liveness` exposed as `pub` in `glyim-borrowck`; `glyim-lower/src/async_state_transform.rs` analysis+planning (`split_at_suspend_points`, `compute_live_across_suspends`, `plan_async_transform`, `plan_resume_arm`, `transform_async_body`) with 6 unit tests; wired into `lower_body` (plan stored in `LowerResult.async_transform`); latent `compute_liveness` out-of-bounds `FixedBitSet` panic fixed.
  - Safety net INTACT & verified (3 tests pass): `single_await_no_state_enum` (single await works through real PipelineCompiler); `multi_await_is_rejected_with_diagnostic` (multi-await emits clear async-v2 compile ERROR, not silent miscompile); `await_in_loop_is_rejected_with_diagnostic` (await-in-loop rejected with actionable message). This satisfies the plan's paramount safety rule: never remove the diagnostic until M4 codegen is real+verified.
  - M4 (real state-machine MIR/HIR codegen: emit `State` enum `Aggregate` + `SwitchInt` + `Poll::Pending`/`Ready`, reshape `FooFuture` at HIR) and M5 (host-run `two_step` runtime proof) are NOT done and explicitly deferred: M4 cannot be runtime-verified on macOS, and M5 is blocked by an async **codegen** gap (generic `Future`/`block_on` instantiation lowers to `TyKind::Error` at LLVM, panicking). Removing the diagnostic now would regress multi-await into a silent infinite-`Pending` miscompile — forbidden by the plan. See `docs/plans/v0.1.0/ASYNC_V1_MIR_PLAN.md`.
- **Phase 5 (P1) Deref autoderef for ADTs — DONE & GREEN.** `DerefRegistry` (`crates/glyim-type/src/deref.rs`) + `TyCtx::deref_ty` (exact + generic-template substitution) + `populate_deref_registry` (`glyim-typeck/src/deref_impl.rs`, invoked from `typeck_crate`) already implemented. Test `impl_deref_for_adt_populates_registry` (typeck) + 2 type `deref_ty` tests pass.

## Next phases (from plan, in priority order)
- **Phase 6 (fix) Range-const materialization — DONE & GREEN.** `cv_const` (`pipeline_context.rs`) `ConstValue::Range` arm folds both bounds into `MirConstKind::Aggregate` (reusing existing backend lowering; no new const kind needed). Test `cv_const_range_folds_to_aggregate` passes (0..10 → Aggregate([Int(0), Int(10)]), no ConstRef).
- **Phase 9 (P3) Inclusive range slicing `..=` — DONE & GREEN.** `lower_dynamic_range_slice` (`lower_rvalue.rs`) adds 1 to the end bound for `..=` with an overflow check routing to `Unreachable` (panic) when `end == usize::MAX`, then normal `start <= end` / `end <= len` bounds checks. 6 honest tests in `dynamic_range_slice.rs` pass (incl. contrast test proving exclusive `..` emits NO `end + 1`).
- **Phase 8 (P2) Proc-macro build orchestration — DONE (logic).** `glyim-cli/src/lib.rs` `build_proc_macro_dependencies` + `compile_proc_macro_dep` compile each dep for the HOST triple to a cdylib, `load_cdylib`, and `Registry::merge` into a combined registry threaded through `compile_file(_with_artifacts)`. `Registry::merge` verified sound. The plan's end-to-end `#[derive]` CLI test was never written — it requires glyim to *author* proc-macro crates (the `proc_macro` TokenStream + `#[proc_macro_derive]` language feature), which is a separate language-level capability outside this phase's stated scope ("What's missing is purely glyim-cli driver orchestration").
- **Phase 7 (P2) Windows SEH funclets — DONE (code) / BLOCKED (validation).** `seh_ffi.rs` (raw LLVM funclet FFI: `LLVMBuildCleanupPad`/`LLVMBuildCleanupRet`/etc.) + `emit_seh_cleanuppad`/`emit_seh_cleanupret` in `lower.rs` are implemented and `glyim-codegen-llvm` compiles cleanly. `emit_landingpad` branches on `Personality::Seh`. **Cannot validate** on this macOS host: the plan requires cross-linking against a real MSVC CRT / `lld-link` on Windows to confirm the funclet token chain actually unwinds — LLVM IR text inspection alone can't catch a malformed pad. No SEH test exists to flip `seh_target_lowers_cleanup_landingpad_green`'s `landingpad`/`resume` assertions to `cleanuppad`/`cleanupret`; that rename + assertion-flip is deferred until MSVC validation is possible.

## Status: all 9 phases implemented to their achievable (host-validatable) extent.
- 6 phases fully green with honest tests: 1, 2, 4, 5, 6, 9.
- Phase 3: v1 green (analysis+planning wired, diagnostic safety net verified); M4/M5 deferred behind the never-removed async-v2 diagnostic (plan's paramount safety rule) — runtime proof blocked by generic-`Future` LLVM codegen gap (`TyKind::Error`) + macOS host.
- Phase 8: orchestration logic green; end-to-end derive test deferred (needs proc-macro authoring language feature).
- Phase 7: code complete + compiles; validation blocked on real MSVC target (unavailable here).

## Full-suite gate
`cargo nextest run --workspace` → 4117 passed (after xref_probe determinism fix).

## Safety rule (plan §P3): never let unsupported-shape detection silently fall through to old skeleton behavior.

## Restart note
- Verify compiler behavior via REAL `PipelineCompiler` path, not standalone `glyim-cli`.
- Do NOT force-enable intentionally-disabled `harness_tests` module.
- Keep individual tool-call arguments under ~8K tokens.
- Plan line numbers are STALE — locate symbols by NAME.
- NEVER include API keys/tokens/passwords/credentials; redact as `[REDACTED]`.
