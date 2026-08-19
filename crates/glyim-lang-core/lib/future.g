//! Asynchronous runtime foundation for the Glyim core library.
//!
//! This module defines the minimal `Future` model required to make
//! `async`/`.await` usable for straight-line, non-concurrent code in its
//! first iteration. The `Waker`/`Context` types are intentionally minimal:
//! a single-threaded, no-op waker is enough to drive futures to completion
//! via a `block_on`-style poll loop. A real multi-threaded waker / I/O
//! reactor is a tracked follow-up (see `KNOWN_GAPS.md` Phase 5).

/// The result of polling a [`Future`].
///
/// `Ready` carries the produced value; `Pending` means the future is not yet
/// complete and must be polled again after its waker is signalled.
pub enum Poll<T> {
    /// The future has completed with a value.
    Ready(T),
    /// The future is not yet complete.
    Pending,
}

/// A handle to a task's waker.
///
/// In this first iteration the waker is a no-op: a single-threaded executor
/// simply keeps polling the future until it returns `Ready`. The data/vtable
/// indirection of a production waker is intentionally omitted for now.
pub struct Waker;

impl Waker {
    /// Wake the associated task. No-op in the single-threaded executor.
    fn wake(&self) {}

    /// Wake the associated task by reference. No-op here.
    fn wake_by_ref(&self) {}
}

/// Per-poll contextual data handed to [`Future::poll`].
///
/// Carries the [`Waker`] the future should use to signal readiness.
pub struct Context {
    waker: Waker,
}

impl Context {
    /// Construct a `Context` from a [`Waker`].
    fn new() -> Context {
        Context { waker: Waker }
    }

    /// Borrow the [`Waker`] associated with this poll.
    fn waker(&self) -> &Waker {
        &self.waker
    }
}

/// A computation that may complete in the future.
///
/// A future is driven by repeatedly calling [`poll`](Future::poll) until it
/// returns [`Poll::Ready`]. Each call resumes the future where it last
/// suspended (at an `.await` point once `async fn` desugaring lands).
pub trait Future {
    /// The type of value produced on completion.
    type Output;

    /// Attempt to resolve the future to a final value, registering the
    /// current task for wake-up if the value is not yet available.
    fn poll(&mut self, cx: &mut Context) -> Poll<Self::Output>;
}
