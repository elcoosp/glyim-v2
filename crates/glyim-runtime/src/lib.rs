//! Runtime support: memory allocation, panic handling, drop glue, ABI stubs.
//!
//! This crate provides the low-level FFI interface used by generated code:
//! - `glyim_alloc` — heap allocation with alignment
//! - `glyim_dealloc` — heap deallocation
//! - `glyim_drop_in_place` — drop glue via caller-provided destructor
//! - `glyim_panic` — unrecoverable panic handler

pub use glyim_core::abi::ALIGN_MAX;

use std::alloc::{self, Layout};

/// Type for a drop function pointer passed to `glyim_drop_in_place`.
///
/// The function receives a pointer to the value to drop and is responsible
/// for calling the type-specific destructor. Generated code produces these
/// by monomorphizing drop glue for each concrete type.
pub type DropFn = unsafe extern "C" fn(*mut u8);

/// Allocate memory with the given size and alignment.
///
/// Returns a pointer to the allocated memory, or null if allocation fails.
/// For zero-size allocations, returns a dangling non-null pointer.
///
/// # Safety
///
/// This is an FFI function intended for use by generated code. The caller
/// must ensure that:
/// - `align` is a valid alignment (a power of two, or zero which is treated as 1)
/// - If `size > 0`, the returned pointer must be deallocated with
///   `glyim_dealloc` using the same `size` and `align`
/// - A null return indicates allocation failure (OOM)
#[unsafe(no_mangle)]
pub extern "C" fn glyim_alloc(size: usize, align: usize) -> *mut u8 {
    if size == 0 {
        return std::ptr::NonNull::dangling().as_ptr();
    }
    let layout = match Layout::from_size_align(size, align.max(1)) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };
    // SAFETY: Layout is validated above. alloc::alloc may return null on OOM,
    // which is a valid return value for this FFI function.
    unsafe { alloc::alloc(layout) }
}

/// Deallocate memory previously allocated by `glyim_alloc`.
///
/// # Safety
///
/// - `ptr` must have been returned by `glyim_alloc` with the same `size` and `align`
/// - `ptr` must not have been already deallocated
/// - The memory pointed to by `ptr` must not be accessed after this call
/// - Passing a null pointer or zero size is safe and results in a no-op
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_dealloc(ptr: *mut u8, size: usize, align: usize) {
    if size == 0 {
        return;
    }
    if ptr.is_null() {
        return;
    }
    let layout = match Layout::from_size_align(size, align.max(1)) {
        Ok(l) => l,
        Err(_) => return,
    };
    // SAFETY: Caller guarantees ptr was allocated with the same layout
    // and has not been deallocated.
    unsafe { alloc::dealloc(ptr, layout) }
}

/// Drop a value in place by calling its type-specific destructor.
///
/// The `drop_fn` parameter is a function pointer that implements the drop
/// glue for the value at `ptr`. Generated code produces these by
/// monomorphizing drop glue for each concrete type. If `drop_fn` is null,
/// the type is trivially destructible and no action is taken.
///
/// # Safety
///
/// - `ptr` must point to a valid, aligned value of the expected type
/// - `drop_fn` must be the correct destructor for the type at `ptr`, or null
///   if the type has no destructor (trivially destructible)
/// - After this call, the value at `ptr` has been dropped; `ptr` must not be
///   used to access the dropped value (but may be passed to `glyim_dealloc`)
/// - Passing a null `ptr` is safe and results in a no-op
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_drop_in_place(ptr: *mut u8, drop_fn: Option<DropFn>) {
    if ptr.is_null() {
        return;
    }
    if let Some(drop) = drop_fn {
        // SAFETY: Caller guarantees that ptr points to valid memory and
        // drop_fn is the correct destructor for the type at ptr.
        unsafe { drop(ptr) }
    }
    // If drop_fn is None, the type is trivially destructible — no action needed.
}

/// Panic handler for the runtime.
///
/// Aborts the process immediately. The `msg` and `len` parameters provide
/// the panic message as a UTF-8 byte slice, but are currently unused.
/// A future implementation will print the message before aborting.
///
/// # Safety
///
/// This is an FFI function intended for use by generated code. It never returns.
/// - `msg` should point to valid UTF-8 data of length `len`
/// - `len` should be the exact byte length of the message
#[unsafe(no_mangle)]
pub extern "C" fn glyim_panic(_msg: *const u8, _len: usize) -> ! {
    std::process::abort()
}



// ========== Networking (TCP) ==========

/// Connect to a TCP server.
///
/// # Safety
/// `addr` must point to a valid UTF-8 string of length `addr_len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_tcp_connect(
    addr: *const u8,
    addr_len: usize,
    port: u16,
) -> i32 {
    tracing::warn!("STUB: glyim_net_tcp_connect");
    -1
}

/// Bind a TCP socket.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_tcp_bind(
    addr: *const u8,
    addr_len: usize,
    port: u16,
) -> i32 {
    tracing::warn!("STUB: glyim_net_tcp_bind");
    -1
}

/// Accept an incoming TCP connection.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_tcp_accept(fd: i32) -> i32 {
    tracing::warn!("STUB: glyim_net_tcp_accept");
    -1
}

/// Read from a TCP socket.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_tcp_read(fd: i32, buf: *mut u8, count: usize) -> isize {
    tracing::warn!("STUB: glyim_net_tcp_read");
    -1
}

/// Write to a TCP socket.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_tcp_write(fd: i32, buf: *const u8, count: usize) -> isize {
    tracing::warn!("STUB: glyim_net_tcp_write");
    -1
}

/// Get local address of a TCP socket.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_tcp_local_addr(
    fd: i32,
    buf: *mut u8,
    buf_len: usize,
) -> i32 {
    tracing::warn!("STUB: glyim_net_tcp_local_addr");
    -1
}

// ========== Networking (UDP) ==========

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_udp_bind(
    addr: *const u8,
    addr_len: usize,
    port: u16,
) -> i32 {
    tracing::warn!("STUB: glyim_net_udp_bind");
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_udp_send_to(
    fd: i32,
    buf: *const u8,
    count: usize,
    dest_addr: *const u8,
    dest_addr_len: usize,
    dest_port: u16,
) -> isize {
    tracing::warn!("STUB: glyim_net_udp_send_to");
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_udp_recv_from(
    fd: i32,
    buf: *mut u8,
    count: usize,
    src_addr: *mut u8,
    src_addr_len: *mut usize,
    src_port: *mut u16,
) -> isize {
    tracing::warn!("STUB: glyim_net_udp_recv_from");
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_udp_connect(
    fd: i32,
    addr: *const u8,
    addr_len: usize,
    port: u16,
) -> i32 {
    tracing::warn!("STUB: glyim_net_udp_connect");
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_udp_send(fd: i32, buf: *const u8, count: usize) -> isize {
    tracing::warn!("STUB: glyim_net_udp_send");
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_net_udp_recv(fd: i32, buf: *mut u8, count: usize) -> isize {
    tracing::warn!("STUB: glyim_net_udp_recv");
    -1
}

// ========== Threading ==========

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_spawn(f: extern "C" fn(*mut u8), arg: *mut u8) -> usize {
    tracing::warn!("STUB: glyim_thread_spawn");
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_join(handle: usize) -> i32 {
    tracing::warn!("STUB: glyim_thread_join");
    -1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_yield() {
    tracing::warn!("STUB: glyim_thread_yield");
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_sleep(secs: u64, nanos: u32) {
    tracing::warn!("STUB: glyim_thread_sleep");
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_park() {
    tracing::warn!("STUB: glyim_thread_park");
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_unpark(handle: usize) {
    tracing::warn!("STUB: glyim_thread_unpark");
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_current_id() -> usize {
    tracing::warn!("STUB: glyim_thread_current_id");
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_thread_available_parallelism() -> usize {
    tracing::warn!("STUB: glyim_thread_available_parallelism");
    0
}

// ========== Time ==========

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_time_now_secs() -> u64 {
    tracing::warn!("STUB: glyim_time_now_secs");
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_time_now_nanos() -> u64 {
    tracing::warn!("STUB: glyim_time_now_nanos");
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_time_system_secs() -> u64 {
    tracing::warn!("STUB: glyim_time_system_secs");
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_time_system_nanos() -> u64 {
    tracing::warn!("STUB: glyim_time_system_nanos");
    0
}
#[cfg(test)]
mod tests;
