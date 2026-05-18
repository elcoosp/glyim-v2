//! Runtime support: memory allocation, panic handling, drop glue, ABI stubs.

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
///
/// # Safety
///
/// - `ptr` must have been returned by `glyim_alloc` with the same `size` and `align`.
/// - `ptr` must not have been already deallocated.
/// - The memory pointed to by `ptr` must not be accessed after this call.
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
    unsafe { alloc::dealloc(ptr, layout) }
}

/// Drop a value in place by calling its drop glue.
///
/// This is a stub implementation that just calls `glyim_dealloc` on the
/// pointer. In a full implementation, this would call the type-specific
/// drop implementation for the value pointed to by `ptr`.
///
/// # Safety
///
/// - `ptr` must point to a valid, aligned value of the expected type.
/// - The value will be dropped and its memory potentially deallocated.
/// - `ptr` must not be used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glyim_drop_in_place(ptr: *mut u8) {
    if !ptr.is_null() {
        // In a full implementation, we would call the type‑specific destructor.
        // For now, just log that drop occurred.
        // SAFETY: The caller guarantees that `ptr` points to valid memory.
        unsafe {
            // Example: call a function pointer from a vtable.
            // This is a stub; real implementation will use a vtable stored
            // in the fat pointer for trait objects or monomorphized drop glue.
            // We'll simply mark the memory as "dropped" by writing a sentinel.
            // This is not correct but prevents double-free in simple cases.
            // Actual drop must call the destructor.
            // Placeholder: write a byte to indicate dropped state.
            core::ptr::write_volatile(ptr, 0u8);
        }
        // Optional: log drop for debugging
        if cfg!(debug_assertions) {
            println!("glyim_drop_in_place called at {:p}", ptr);
        }
    }
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
        unsafe {
            glyim_dealloc(ptr, 8, 8);
        }
    }

    #[test]
    fn test_alloc_zero_size() {
        let ptr = glyim_alloc(0, 1);
        unsafe {
            glyim_dealloc(ptr, 0, 1);
        }
    }

    #[test]
    fn test_alloc_large_alignment() {
        let ptr = glyim_alloc(64, 16);
        assert!(!ptr.is_null());
        let addr = ptr as usize;
        assert_eq!(addr % 16, 0, "allocated memory must be aligned");
        unsafe {
            glyim_dealloc(ptr, 64, 16);
        }
    }

    #[test]
    fn test_alloc_multiple_sizes() {
        for &size in &[1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] {
            let ptr = glyim_alloc(size, 8);
            assert!(!ptr.is_null(), "alloc of size {} failed", size);
            unsafe {
                std::ptr::write_bytes(ptr, 0xAA, size);
                glyim_dealloc(ptr, size, 8);
            }
        }
    }

    #[test]
    fn test_dealloc_null_is_safe() {
        unsafe {
            glyim_dealloc(std::ptr::null_mut(), 8, 8);
        }
    }

    #[test]
    fn test_dealloc_zero_size_is_safe() {
        let ptr = glyim_alloc(0, 1);
        unsafe {
            glyim_dealloc(ptr, 0, 1);
        }
    }

    #[test]
    fn test_alloc_alignment_respected() {
        for &align in &[1, 2, 4, 8, 16, 32, 64] {
            let ptr = glyim_alloc(64, align);
            assert!(!ptr.is_null(), "alloc with align {} failed", align);
            let addr = ptr as usize;
            assert_eq!(
                addr % align,
                0,
                "allocated memory must be aligned to {}",
                align
            );
            unsafe {
                glyim_dealloc(ptr, 64, align);
            }
        }
    }

    #[test]
    fn test_drop_in_place_is_callable() {
        let ptr = glyim_alloc(8, 8);
        assert!(!ptr.is_null());
        unsafe {
            let typed = ptr as *mut u64;
            typed.write(12345u64);
        }
        // glyim_drop_in_place is a stub; it should not crash
        unsafe {
            glyim_drop_in_place(ptr);
        }
        // After drop_in_place, memory is logically invalid but still accessible
        // in this stub implementation. Dealloc to clean up.
        unsafe {
            glyim_dealloc(ptr, 8, 8);
        }
    }

    #[test]
    fn test_drop_in_place_null_pointer() {
        // Should not crash even with null
        unsafe {
            glyim_drop_in_place(std::ptr::null_mut());
        }
    }

    #[test]
    fn test_alloc_write_read_roundtrip() {
        let ptr = glyim_alloc(std::mem::size_of::<u32>(), std::mem::align_of::<u32>());
        assert!(!ptr.is_null());
        unsafe {
            let typed = ptr as *mut u32;
            typed.write(0xDEADBEEFu32);
            assert_eq!(*typed, 0xDEADBEEFu32);
            glyim_dealloc(ptr, std::mem::size_of::<u32>(), std::mem::align_of::<u32>());
        }
    }

    #[test]
    fn test_alloc_many_small_allocations() {
        let mut ptrs = Vec::new();
        for i in 0..100 {
            let ptr = glyim_alloc(8, 8);
            assert!(!ptr.is_null(), "allocation {} failed", i);
            unsafe {
                let typed = ptr as *mut u64;
                typed.write(i as u64);
            }
            ptrs.push(ptr);
        }
        // Verify all values are still intact
        for (i, &ptr) in ptrs.iter().enumerate() {
            unsafe {
                let typed = ptr as *mut u64;
                assert_eq!(*typed, i as u64, "value at allocation {} corrupted", i);
            }
        }
        // Clean up
        for &ptr in &ptrs {
            unsafe {
                glyim_dealloc(ptr, 8, 8);
            }
        }
    }

    #[test]
    fn test_alloc_and_drop_in_place_sequence() {
        // Simulates the pattern: allocate, write, drop_in_place, dealloc
        for i in 0..10 {
            let ptr = glyim_alloc(16, 8);
            assert!(!ptr.is_null(), "allocation {} failed", i);
            unsafe {
                std::ptr::write_bytes(ptr, i as u8, 16);
            }
            unsafe {
                glyim_drop_in_place(ptr);
            }
            unsafe {
                glyim_dealloc(ptr, 16, 8);
            }
        }
    }

    #[test]
    fn test_drop_in_place_multiple_calls() {
        // Call drop_in_place multiple times on different allocations
        for _ in 0..50 {
            let ptr = glyim_alloc(32, 8);
            assert!(!ptr.is_null());
            unsafe {
                std::ptr::write_bytes(ptr, 0xFF, 32);
            }
            unsafe {
                glyim_drop_in_place(ptr);
            }
            unsafe {
                glyim_dealloc(ptr, 32, 8);
            }
        }
    }

    #[test]
    fn test_alloc_dealloc_varying_sizes() {
        // Test allocation and deallocation with various sizes
        for size in [
            1, 2, 4, 7, 8, 13, 16, 31, 32, 63, 64, 127, 128, 255, 256, 512, 1024, 4096,
        ] {
            let ptr = glyim_alloc(size, 8);
            assert!(!ptr.is_null(), "alloc of size {} failed", size);
            unsafe {
                std::ptr::write_bytes(ptr, 0x42, size);
            }
            unsafe {
                glyim_drop_in_place(ptr);
            }
            unsafe {
                glyim_dealloc(ptr, size, 8);
            }
        }
    }

    #[test]
    fn test_alloc_dealloc_interleaved() {
        // Interleave allocations and deallocations to test heap consistency
        let mut live = Vec::new();
        for i in 0..100 {
            if i % 3 == 0 && !live.is_empty() {
                // Deallocate the oldest
                let (ptr, size) = live.remove(0);
                unsafe {
                    glyim_drop_in_place(ptr);
                }
                unsafe {
                    glyim_dealloc(ptr, size, 8);
                }
            } else {
                let size = 8 + (i % 4) * 8;
                let ptr = glyim_alloc(size, 8);
                assert!(!ptr.is_null());
                unsafe {
                    std::ptr::write_bytes(ptr, (i % 256) as u8, size);
                }
                live.push((ptr, size));
            }
        }
        // Clean up remaining
        for (ptr, size) in live {
            unsafe {
                glyim_drop_in_place(ptr);
            }
            unsafe {
                glyim_dealloc(ptr, size, 8);
            }
        }
    }
}
