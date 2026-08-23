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
- **Why**: A correct multi-await state machine must store the suspended future's
  concrete type and resume at the correct CFG point. That requires (a) fixing the
  single-await path end-to-end first, then (b) a post-type-check pass reusing
  `glyim-borrowck::compute_liveness`. The plan (`GLYIM_DESTUB_PLAN.md` §Phase 3)
  scopes this as "the single largest item... its own project, not a quick patch".
- **Remediation**: tracked sub-project `ASYNC_V1_MIR_PLAN.md`. M0–M3 shipped
  (expose `compute_liveness`; `async_state_transform` analysis module with 6
  unit tests on synthetic MIR; wired into `lower_body`; plus a real `compute_liveness`
  OOB-panic fix). M4 (apply the state-machine codegen) and M5 (host-run `two_step`
  proof) remain. M4 first requires the single-await path to actually compile
  (prerequisite blockers above), then the multi-await codegen, neither of which
  can be completed green in this session.
- **Status**: M0–M3 done + green; M4/M5 blocked on prerequisite async machinery
  (single-await must compile first) and on host-infeasible runtime validation (M5).
