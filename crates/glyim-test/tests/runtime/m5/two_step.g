// M5 (async v1) — multi-await shape MUST NOT silently miscompile.
//
// `two_step` awaits twice (`dep(a).await; dep(b).await`). Per the plan's safety
// rule (GLYIM_DESTUB_PLAN.md Phase 3) this shape must emit the `async-v2`
// diagnostic (error 61) and NOT fall through to a silently-broken state
// machine. This fixture asserts the honest compile-FAIL behavior via `//~ ERROR`
// annotations: the desugar must surface the diagnostic, not a miscompiled binary.
//
// GATED TO LINUX: the pipeline links a native x86_64-unknown-linux-gnu binary,
// so the compile-check only runs on that target. When M4's real multi-await
// codegen lands and is runtime-verified (M5), this fixture flips to run-pass with
// `// check-stdout: 3`.
//
// test-mode: compile-fail
// only-target: x86_64-unknown-linux-gnu
//
// (The M5 driver asserts the emitted async-v2 diagnostic text directly rather
// than via `//~` annotations, because that diagnostic carries a DUMMY span and
// cannot be pinned to a source line.)
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
