//! Runtime support: memory allocation, panic handling, ABI stubs.

pub use glyim_core::abi::ALIGN_MAX;

use std::alloc::{self, Layout};

/// Allocate memory with the given size and alignment.
/// Returns a pointer to the allocated memory, or null if allocation fails.
#[unsafe(no_mangle)]
pub extern "C" fn glyim_alloc(size: usize, align: usize) -> *mut u8 {
    if size == 0 {
        return std::ptr::NonNull::dangling().as_ptr();
    }
    let layout = match Layout::from_size_align(size, align.max(1)) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };
    unsafe { alloc::alloc(layout) }
}

/// Deallocate memory previously allocated by `glyim_alloc`.
#[unsafe(no_mangle)]
pub extern "C" fn glyim_dealloc(ptr: *mut u8, size: usize, align: usize) {
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
    unsafe { alloc::dealloc(ptr, layout) }
}

/// Panic handler for the runtime.
#[unsafe(no_mangle)]
pub extern "C" fn glyim_panic(_msg: *const u8, _len: usize) -> ! {
    std::process::abort()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alloc_returns_non_null() {
        let ptr = glyim_alloc(8, 8);
        assert!(
            !ptr.is_null(),
            "glyim_alloc must return non-null for valid requests"
        );
        unsafe {
            let typed = ptr as *mut u64;
            typed.write(42u64);
            assert_eq!(*typed, 42u64);
        }
        glyim_dealloc(ptr, 8, 8);
    }

    #[test]
    fn test_alloc_zero_size() {
        let ptr = glyim_alloc(0, 1);
        glyim_dealloc(ptr, 0, 1);
    }

    #[test]
    fn test_alloc_large_alignment() {
        let ptr = glyim_alloc(64, 16);
        assert!(!ptr.is_null());
        let addr = ptr as usize;
        assert_eq!(addr % 16, 0, "allocated memory must be aligned");
        glyim_dealloc(ptr, 64, 16);
    }
}
