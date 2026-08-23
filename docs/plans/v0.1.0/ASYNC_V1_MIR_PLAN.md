# Async v1 — MIR resume-dispatch (tracked sub-project)

Status: IN PROGRESS (authorized 2026-08-23).
Source plan: `GLYIM_DESTUB_PLAN.md` §Phase 3 (lines 357–534).
The de-stub plan scoped this as "the single largest item... its own project, not a quick patch."

## Goal
Make `async fn` with multiple sequential top-level `.await`s actually work:
suspend, return `Poll::Pending`, and resume at the correct point on the next
`poll()` call — instead of the current `Pending => panic` skeleton (or, for
unhandled shapes, the clear `async-v2` diagnostic).

## Approach (per plan §6.1)
Transform at MIR (post-type-check), reusing `glyim-borrowck::compute_liveness`.
The async body is lowered to MIR *before* this pass runs: the HIR `desugar_async`
already rewrites `.await` into a `match fut.poll(...) { Ready(v) => v, Pending => ... }`,
so at MIR every `.await` shows up as a `Call` terminator to `Future::poll`
(the suspend point). The transform:

1. `split_at_suspend_points(body)` — locate every `poll()` `Call` terminator;
   these are the suspend sites. Record `(block, destination, callee)` per site.
2. `compute_live_across_suspends(body, sites)` — for each suspend site `k`,
   compute the set of locals live *after* site `k` (via `compute_liveness`).
   These become the `S_k` state-variant fields.
3. `build_state_store_and_return_pending(...)` — build the `State` enum
   (`Start, S0, S1, ... S_{n-1}, Done`) plus, inside `poll()`, a
   `match self.state { ... }` dispatch:
   - `Start`/`S_k`: run the body segment up to/after site `k`; on the
     `Pending` arm of the `match fut.poll()`, store live locals into the next
     `S_{k+1}` variant and `return Poll::Pending`; on `Ready(v)` bind `v` and
     continue to the next segment.
   - `Done`: `return Poll::Ready(<tail>)`.
4. `rewrite_resume_arm(...)` — fix the `Pending` arm of each site's `match` to
   store state + live locals and return `Pending` instead of panicking.

## Milestones (tracked)

- [x] M0 — Repo reality check: confirmed `glyim-mir` has `Body`/`LocalDecl{ty}`;
      `.await` reaches MIR as `poll()` `Call`; `Future`/`Poll`/`block_on` runtime
      exists (`glyim-lang-core/lib/future.g`, `glyim-runtime/src/async_runtime.rs`);
      `glyim-borrowck::compute_liveness` is `pub(crate)` (needs `pub` expose).
- [x] M1 — Expose `compute_liveness` as `pub` in `glyim-borrowck` (committed
      133653bb).
- [x] M2 — `glyim-lower/src/async_state_transform.rs`: real `split_at_suspend_points`
      + `SuspendSite` + `AsyncTransformPlan` (Start/S0..S_{n-1}/Done) +
      `plan_resume_arm` + `transform_async_body`, operating on `glyim_mir::Body`.
      6 unit tests on synthetic MIR bodies pass (committed 133653bb).
- [x] M3 — Wire the transform into the lowering pipeline: `lower_body` runs
      `transform_async_body` after MIR gen and stores the plan in
      `LowerResult.async_transform`. Also FIXED a latent `compute_liveness`
      out-of-bounds `FixedBitSet::insert` panic exposed by the wiring (MIR
      bodies can reference locals >= `body.locals.len()`). All 218
      glyim-lower tests + full workspace (73 binaries) green (committed
      7023de78).
- [ ] M4 — `desugar_async`: route multi-await sequential bodies through the
      REAL state-machine codegen (emit the `State` enum `Aggregate` + discriminant
      `SwitchInt` + `Poll::Pending`/`Ready` returns into the MIR `Body`, and
      reshape `FooFuture`'s fields at the HIR desugar stage). This is the heavy
      half: it must reshape the `FooFuture` ADT's fields/types (HIR-level), not
      just MIR, and CANNOT be runtime-verified on this macOS host. The `async-v2`
      diagnostic remains the safety net until this codegen is real + verified —
      removing it now would regress multi-await into a silent miscompile (violates
      the plan's paramount safety rule).
- [ ] M5 — Host-run `two_step` proof (compile async fn, `block_on`, assert result
      == a+b, poll()>1). BLOCKED on host (macOS cannot run the Linux-gated
      glyim binary/executor chain); tracked here as the final verification, not
      faked.

## Status (2026-08-24)
M0–M3 shipped and green. The single-await **end-to-end path is now UNBLOCKED
and verified**: nested single-await (`one_step` awaiting `ready`) compiles
through the real `PipelineCompiler` with zero diagnostics (regression test
`nested_async_single_await_compiles`). Two typechecker bugs were fixed —
top-level fn signatures must be registered before impl-body checking, and
method-dispatch candidate probing must not emit diagnostics on non-matching
impls. M4/M5 remain the heavy half: the multi-await codegen requires reshaping
`FooFuture` at HIR (not MIR alone), and the runtime proof cannot execute on this
host. The `async-v2` diagnostic (errors 60/61) stays as the honest safety net for
multi-await until M4 lands verified.

## Known constraints
- The full end-to-end `two_step` runtime test (M5) cannot be verified on this
  macOS host; the plan's runtime validation was "never actually exercised".
- M2/M3 are the verifiable core; M5 is verification-only and remains blocked.
- Do NOT ship a silent miscompile: if a shape can't be transformed, keep the
  clear `async-v2` diagnostic.
