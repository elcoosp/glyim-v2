# Known Gaps

Tracked gaps for the Glyim v0.1.0 de-stubbing effort. Each entry: what is
missing, why, and the remediation tracking.

## async-v2 — async multi-await resume-dispatch (MIR transform)

- **What**: `async fn` with more than one sequential top-level `.await` currently
  does NOT suspend/resume correctly. Loop-await and multi-await are rejected at
  `desugar_async` with a clear diagnostic (errors 60/61) — NOT a silent
  miscompile.
- **Empirical state (verified 2026-08-23 via `PipelineCompiler`)**: even the
  **single-await** path does NOT compile end-to-end. Probing a real
  `async fn one_step(a) { let x = ready(a).await; x + 1 }` through the full
  pipeline yields 9 diagnostics: "enum-variant value paths are not yet
  supported", "no method `poll` found for type", "trait `Future` is not
  implemented", "unresolved name `panic`", etc. The existing `desugar_one_async_fn`
  emits a poll `Match` + `Pending => panic` skeleton, but the surrounding async
  machinery (enum-variant value paths, `poll` method resolution, real future
  types, `panic` builtin) is not yet wired, so the body does not type-check.
  The `desugar_async_fn_compiles` test only passes because its `add_one` has
  **no actual `.await`** — it exercises the `async fn` declaration form, not the
  await desugar. So the async feature is broadly non-functional end-to-end even
  for the "supported" single-await shape.
- **Prerequisite blocker triage (2026-08-23, after driving the real compiler)**:
  Isolated which of the 9 diagnostics are general vs async-specific by probing
  each construct through the pipeline in isolation:
  - **`panic` unresolved** — GENERAL bug (no `panic` builtin/intrinsic). The
    desugar emitted `Pending => panic!()` as a bare function call, but `panic`
    is a *macro* (`panic.g` → `panic_any(...) -> !`), not a resolvable `FnDef`.
    **FIXED (stopgap)**: the desugar now emits `Pending => loop {}` (a diverging
    expression that unifies with `Poll<_>`). This removes the `panic`` symptom
    for the single-await shape; a genuine `Pending` should suspend/resume (M4).
  - **`let`-bound locals unresolved inside impl-method bodies** — GENERAL
    typechecker name-resolution bug, independent of async. `check_pattern`
    bound the `let` name under the *remapped* (TyCtx-interner) `Name`, while
    later references resolve via the original HIR `Name`; when those two
    interners differ (impl-method bodies), the binding is invisible →
    "unresolved name `x`". Verified: `let x = self.f0; x + 1` in a trait-impl
    `poll` body failed before, passes after. **FIXED**: `check_pattern` now also
    binds the original HIR `Name` (sharing one `LocalVarId`), so lookups via
    either interner succeed. Full workspace stays green.
  - **`self` typed as `<error>` in the nested-async case** — the REMAINING
    blocker. A single async fn with NO `.await` (`add_one`) desugars and
    type-checks fine. But an async fn that calls ANOTHER async fn
    (`one_step` → `let x = ready(a).await` → `ready(self.f0).poll()`) still
    yields `&mut <error> vs Adt...` and "no method `poll` found", i.e. `self`
    inside the desugared `poll` body is `<error>`. Root cause is an
    HIR-desugar ↔ def-map/typechecker integration gap: the desugar appends
    `OneStepFuture`/`ReadyFuture` items to `hir.items`, but the typechecker's
    def map / ADT registry / interner bridge does not resolve those generated
    future types (and the `self` of the generated impl method) when an async fn
    is *nested inside* another. This is deeper than a two-line fix and cannot be
  runtime-verified on this host.
- **Why**: A correct multi-await state machine must store the suspended future's
  concrete type and resume at the correct CFG point. That requires (a) fixing the
  single-await path end-to-end first, then (b) a post-type-check pass reusing
  `glyim-borrowck::compute_liveness`. The plan (`GLYIM_DESTUB_PLAN.md` §Phase 3)
  scopes this as "the single largest item... its own project, not a quick patch".
- **Remediation**: tracked sub-project `ASYNC_V1_MIR_PLAN.md`. M0–M3 shipped
  (expose `compute_liveness`; `async_state_transform` analysis module with 6
  unit tests on synthetic MIR; wired into `lower_body`; plus a real `compute_liveness`
  OOB-panic fix). M4 (apply the state-machine codegen) and M5 (host-run `two_step`
  proof) remain. Single-await is now PARTIALLY unblocked (panic + let-in-impl
  fixed) but still blocked on the nested-async `self`/generated-type resolution
  gap above.
- **Status**: M0–M3 done + green; single-await partially unblocked (2 of 3
  blockers fixed, 1 deeper blocker remains); M4/M5 blocked on the remaining
  single-await blocker + host-infeasible runtime validation (M5).
