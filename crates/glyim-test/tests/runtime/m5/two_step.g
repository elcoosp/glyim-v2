// M5 (async v1) — multi-await end-to-end runtime proof.
//
// `two_step` awaits twice (`dep(a).await; dep(b).await`). With M4's real
// HIR state-machine desugar (GLYIM_DESTUB_PLAN.md Phase 3) this shape now
// compiles cleanly: `desugar_multi_async_fn` builds a `Start`/`S0`/`S1`/`Done`
// enum and a `poll` body that drives each suspended future and stores the
// in-flight future + live locals across `Poll::Pending`, so the future
// genuinely suspends and resumes. `two_step(1, 2)` => x=dep(1)=1, y=dep(2)=2,
// x+y => 3.
//
// GATED TO LINUX: the pipeline links a native x86_64-unknown-linux-gnu binary,
// so this only executes on that target (macOS/Windows runners Ignore it).
//
// KNOWN BLOCKER (tracked, NOT faked): as of 2026-08-26 the LLVM backend (and
// the MIR interpreter) do not resolve trait-method dispatch (`f.poll()` where
// `f: impl Future`), so this fixture currently cannot produce a runnable
// binary on the host. It encodes the *target* behavior (check-stdout: 3) so
// that once the trait-instantiation gap is closed, `cargo test -p glyim-test
// --test runtime` (on ubuntu-latest) will exercise the full runtime proof
// automatically. The M4 compile-correctness proof (zero diagnostics + a real
// suspend/resume state-machine MIR) is already enforced by the
// `glyim-pipeline` `async_multi_await_runtime` test and the `glyim-typeck`
// `multi_await_compiles_cleanly` test.
//
// test-mode: run-pass
// only-target: x86_64-unknown-linux-gnu
// check-stdout: 3
enum Poll<T> { Ready(T), Pending }
trait Future {
    type Output;
    fn poll(&mut self) -> Poll<Self::Output>;
}
fn block_on<F: Future>(mut f: F) -> F::Output {
    loop {
        match f.poll() {
            Poll::Ready(v) => return v,
            Poll::Pending => { }
        }
    }
}
async fn dep(x: i32) -> i32 { x }
async fn two_step(a: i32, b: i32) -> i32 {
    let x = dep(a).await;
    let y = dep(b).await;
    x + y
}
fn main() -> i32 {
    let f = two_step(1, 2);
    block_on(f)
}
