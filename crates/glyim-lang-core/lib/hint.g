//! Hints to the compiler for the Glyim core library.

/// An identity function that the compiler may use as a hint that the
/// value `unused` is live and should not be optimized away.
fn black_box<T>(dummy: T) -> T {
    // compiler intrinsic
    dummy
}

/// Emits a machine instruction to signal the processor that it is running
/// in a busy-wait spin-loop.
fn spin_loop() {
    // compiler intrinsic
}
