//! Tests for glyim_alloc FFI function.
//!
//! S06-T01: glyim_alloc returns non-null aligned pointer.

use crate::glyim_alloc;

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
        crate::glyim_dealloc(ptr, 8, 8);
    }
}

#[test]
fn test_alloc_zero_size() {
    let ptr = glyim_alloc(0, 1);
    // Zero-size allocation returns a dangling (non-null) pointer
    assert!(
        !ptr.is_null(),
        "zero-size alloc should return dangling pointer"
    );
    unsafe {
        crate::glyim_dealloc(ptr, 0, 1);
    }
}

#[test]
fn test_alloc_large_alignment() {
    let ptr = glyim_alloc(64, 16);
    assert!(!ptr.is_null());
    let addr = ptr as usize;
    assert_eq!(addr % 16, 0, "allocated memory must be aligned");
    unsafe {
        crate::glyim_dealloc(ptr, 64, 16);
    }
}

#[test]
fn test_alloc_multiple_sizes() {
    for &size in &[1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] {
        let ptr = glyim_alloc(size, 8);
        assert!(!ptr.is_null(), "alloc of size {} failed", size);
        unsafe {
            std::ptr::write_bytes(ptr, 0xAA, size);
            crate::glyim_dealloc(ptr, size, 8);
        }
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
            crate::glyim_dealloc(ptr, 64, align);
        }
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
        crate::glyim_dealloc(ptr, std::mem::size_of::<u32>(), std::mem::align_of::<u32>());
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
            crate::glyim_dealloc(ptr, 8, 8);
        }
    }
}

#[test]
fn test_alloc_zero_alignment_treated_as_one() {
    // Alignment 0 is treated as 1 by glyim_alloc, so this should succeed
    let ptr = glyim_alloc(8, 0);
    assert!(!ptr.is_null(), "alignment 0 should be treated as 1");
    unsafe {
        crate::glyim_dealloc(ptr, 8, 0);
    }
}
