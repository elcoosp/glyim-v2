# Known Gaps

Tracked gaps for the Glyim v0.1.0 de-stubbing effort. Each entry: what is
missing, why, and the remediation tracking.

## async-v2 — async multi-await resume-dispatch (MIR transform)

- **What**: `async fn` with more than one sequential top-level `.await` currently
  does NOT suspend/resume correctly. Single-await works (resolves on first poll);
  loop-await and multi-await are rejected at `desugar_async` with a clear
  diagnostic (errors 60/61) — NOT a silent miscompile.
- **Why**: A correct multi-await state machine must store the suspended future's
  concrete type and resume at the correct CFG point. That requires a post-type-
  check MIR pass (`glyim-lower/src/async_state_transform.rs`) reusing
  `glyim-borrowck::compute_liveness`. The plan (`GLYIM_DESTUB_PLAN.md` §Phase 3)
  scopes this as "the single largest item... its own project, not a quick patch".
- **Remediation**: tracked sub-project `ASYNC_V1_MIR_PLAN.md`. M0 (repo reality
  check) + M1 (expose `compute_liveness`) + M2 (transform module, unit-tested on
  synthetic MIR) are the verifiable core. Host-run `two_step` proof (M5) is
  blocked on this macOS host (cannot run the Linux-gated glyim binary/executor).
- **Status**: IN PROGRESS.
