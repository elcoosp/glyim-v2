// M5 (async v1) — single-await end-to-end runtime proof.
//
// Compiles the supported single-await shape through the real pipeline and runs
// it: `add_one(41)` must return 42 via block_on. This fixture is the verified
// supported subset of M5 (the desugar type-checks with zero diagnostics via the
// `PipelineCompiler`; see `nested_async_single_await_compiles` / `desugar_async_fn_compiles`).
//
// GATED TO LINUX: the pipeline links a native x86_64-unknown-linux-gnu binary,
// so this only executes on that target (macOS/Windows runners Ignore it).
//
// KNOWN BLOCKER (tracked, NOT faked): as of 2026-08-24 the single-await path
// still panics at LLVM codegen with `TyKind::Error` (an async codegen gap in
// generic `Future`/`block_on` instantiation), so this fixture currently does
// not produce a runnable binary. It encodes the *target* behavior so that once
// the codegen gap is closed, `cargo test -p glyim-test --test runtime` (on
// ubuntu-latest) will exercise the full runtime proof automatically.
// test-mode: run-pass
// only-target: x86_64-unknown-linux-gnu
// check-stdout: 42

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
async fn add_one(x: i32) -> i32 { x + 1 }
fn main() -> i32 {
    let f = add_one(41);
    block_on(f)
}
