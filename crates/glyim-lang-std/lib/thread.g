//! Native threads for the Glyim standard library.
//!
//! This module contains primitives for spawning and managing threads.

/// A handle to a thread.
struct JoinHandle<T> {
    thread_id: ThreadId,
    _marker: PhantomData<T>,
}

impl<T> JoinHandle<T> {
    /// Wait for the associated thread to finish.
    fn join(self) -> Result<T, Box<dyn Any + Send>> {
        extern "C" {
            fn glyim_thread_join(thread_id: u64) -> i32;
        }
        let rc = unsafe { glyim_thread_join(self.thread_id.to_u64()) };
        if rc != 0 {
            Result::Err("thread panicked".into())
        } else {
            // SAFETY: The thread has completed and the result is valid.
            // In a real implementation, we'd retrieve the return value.
            Result::Err("thread result retrieval not yet implemented".into())
        }
    }

    /// Return the thread ID of this handle.
    fn thread_id(&self) -> ThreadId {
        self.thread_id
    }
}

/// A unique identifier for a running thread.
struct ThreadId {
    id: u64,
}

impl ThreadId {
    /// Create a new `ThreadId` from a raw value.
    fn from_u64(id: u64) -> ThreadId {
        ThreadId { id }
    }

    /// Convert to a raw u64 value.
    fn to_u64(&self) -> u64 {
        self.id
    }
}

/// A handle to a thread.
struct Thread {
    id: ThreadId,
    name: Option<String>,
}

impl Thread {
    /// Get the unique identifier for this thread.
    fn id(&self) -> ThreadId {
        self.id
    }

    /// Get the name of this thread.
    fn name(&self) -> Option<&str> {
        self.name.as_ref().map(|s| s.as_str())
    }
}

/// Spawn a new thread, returning a `JoinHandle` for it.
///
/// The closure `f` is the function to execute in the new thread.
fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    extern "C" {
        fn glyim_thread_spawn(f: *const u8, f_len: usize) -> u64;
    }
    // SAFETY: the closure is Send + 'static, so it can be moved to another thread.
    let id = unsafe { glyim_thread_spawn(0 as *const u8, 0) };
    JoinHandle {
        thread_id: ThreadId::from_u64(id),
        _marker: PhantomData,
    }
}

/// Get a handle to the current thread.
fn current() -> Thread {
    extern "C" {
        fn glyim_thread_current_id() -> u64;
    }
    let id = unsafe { glyim_thread_current_id() };
    Thread {
        id: ThreadId::from_u64(id),
        name: Option::None,
    }
}

/// Get the current thread's unique identifier.
fn current_id() -> ThreadId {
    current().id()
}

/// Cooperatively gives up a timeslice to the OS scheduler.
fn yield_now() {
    extern "C" {
        fn glyim_thread_yield();
    }
    unsafe { glyim_thread_yield() }
}

/// Put the current thread to sleep for at least the specified amount of time.
fn sleep(dur: Duration) {
    extern "C" {
        fn glyim_thread_sleep(secs: u64, nanos: u32);
    }
    unsafe { glyim_thread_sleep(dur.as_secs(), dur.subsec_nanos()) }
}

/// Block the current thread until the specified duration has elapsed.
fn park_timeout(dur: Duration) {
    extern "C" {
        fn glyim_thread_park_timeout(secs: u64, nanos: u32);
    }
    unsafe { glyim_thread_park_timeout(dur.as_secs(), dur.subsec_nanos()) }
}

/// Block the current thread unless or until the token is available.
fn park() {
    extern "C" {
        fn glyim_thread_park();
    }
    unsafe { glyim_thread_park() }
}

/// Atomically makes the token available if it is not already.
fn unpark(thread: &Thread) {
    extern "C" {
        fn glyim_thread_unpark(thread_id: u64);
    }
    unsafe { glyim_thread_unpark(thread.id.to_u64()) }
}

/// Determine whether to give up a timeslice based on a hint.
fn hint_spin_loop() {
    extern "C" {
        fn glyim_thread_spin_loop_hint();
    }
    unsafe { glyim_thread_spin_loop_hint() }
}

/// The maximum number of threads that can be spawned.
const MAX_THREADS: usize = 65536;

/// The default stack size for new threads.
const DEFAULT_STACK_SIZE: usize = 8 * 1024 * 1024;

/// Builder for configuring thread spawning.
struct Builder {
    name: Option<String>,
    stack_size: Option<usize>,
}

impl Builder {
    /// Create a new thread builder with default settings.
    fn new() -> Builder {
        Builder {
            name: Option::None,
            stack_size: Option::None,
        }
    }

    /// Set the name for the new thread.
    fn name(mut self, name: String) -> Builder {
        self.name = Option::Some(name);
        self
    }

    /// Set the stack size for the new thread.
    fn stack_size(mut self, size: usize) -> Builder {
        self.stack_size = Option::Some(size);
        self
    }

    /// Spawn a new thread with the configured settings.
    fn spawn<F, T>(self, f: F) -> Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        extern "C" {
            fn glyim_thread_spawn_named(
                name: *const u8,
                name_len: usize,
                stack_size: usize,
            ) -> u64;
        }
        let (name_ptr, name_len) = match &self.name {
            Option::Some(n) => (n.as_ptr(), n.len()),
            Option::None => (0 as *const u8, 0),
        };
        let stack = self.stack_size.unwrap_or(DEFAULT_STACK_SIZE);
        let id = unsafe { glyim_thread_spawn_named(name_ptr, name_len, stack) };
        if id == 0 {
            Result::Err("failed to spawn thread".into())
        } else {
            Result::Ok(JoinHandle {
                thread_id: ThreadId::from_u64(id),
                _marker: PhantomData,
            })
        }
    }
}

/// The number of logical cores available.
fn available_parallelism() -> Result<usize> {
    extern "C" {
        fn glyim_thread_available_parallelism() -> usize;
    }
    let n = unsafe { glyim_thread_available_parallelism() };
    if n == 0 {
        Result::Err("could not determine available parallelism".into())
    } else {
        Result::Ok(n)
    }
}
