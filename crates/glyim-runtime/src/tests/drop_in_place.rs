//! Tests for glyim_drop_in_place FFI function.

use crate::{glyim_alloc, glyim_dealloc, glyim_drop_in_place};

#[test]
fn drop_in_place_is_callable() {
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
    // Clean up (note: drop_in_place is a stub that doesn't actually drop,
    // so we still need to dealloc)
    unsafe {
        glyim_dealloc(ptr, 8, 8);
    }
}

#[test]
fn drop_in_place_null_pointer() {
    // Should not crash even with null
    unsafe {
        glyim_drop_in_place(std::ptr::null_mut());
    }
}

#[test]
fn drop_in_place_sequence() {
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
fn drop_in_place_multiple_calls() {
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
fn drop_in_place_does_not_corrupt_memory() {
    // The stub implementation should NOT write to the memory
    let ptr = glyim_alloc(std::mem::size_of::<u64>(), std::mem::align_of::<u64>());
    assert!(!ptr.is_null());
    unsafe {
        let typed = ptr as *mut u64;
        typed.write(0xBEEFCAFEu64);
        glyim_drop_in_place(ptr);
        // After drop_in_place (stub), the value should still be intact
        // since the stub doesn't actually drop anything
        assert_eq!(
            *typed, 0xBEEFCAFEu64,
            "drop_in_place stub should not corrupt memory"
        );
    }
    unsafe {
        glyim_dealloc(ptr, std::mem::size_of::<u64>(), std::mem::align_of::<u64>());
    }
}
