use crate::alloc::{GlobalAlloc, Layout};

/// A raw, untyped memory buffer with a capacity.
pub struct RawVec<T> {
    ptr: *mut T,
    cap: usize,
}

impl<T> RawVec<T> {
    /// Creates a new empty `RawVec`.
    pub const fn new() -> Self {
        RawVec {
            ptr: core::ptr::null_mut(),
            cap: 0,
        }
    }

    /// Grows the buffer to have at least `required_cap` capacity.
    pub fn reserve(&mut self, required_cap: usize) {
        if self.cap >= required_cap {
            return;
        }
        let new_cap = required_cap.next_power_of_two().max(8);
        let layout = Layout::from_size_align(
            new_cap * core::mem::size_of::<T>(),
            core::mem::align_of::<T>(),
        )
        .expect("RawVec layout invalid");
        let new_ptr = crate::alloc::GLOBAL.alloc(layout) as *mut T;
        if new_ptr.is_null() {
            crate::alloc::handle_alloc_error(layout);
        }
        if !self.ptr.is_null() {
            unsafe {
                core::ptr::copy_nonoverlapping(self.ptr, new_ptr, self.cap);
            }
            let old_layout = Layout::from_size_align(
                self.cap * core::mem::size_of::<T>(),
                core::mem::align_of::<T>(),
            )
            .expect("Old layout invalid");
            unsafe { crate::alloc::GLOBAL.dealloc(self.ptr as *mut u8, old_layout) };
        }
        self.ptr = new_ptr;
        self.cap = new_cap;
    }

    pub fn as_ptr(&self) -> *const T {
        self.ptr
    }

    pub fn as_mut_ptr(&self) -> *mut T {
        self.ptr
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }
}

impl<T> Default for RawVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for RawVec<T> {
    fn drop(&mut self) {
        if self.cap == 0 || self.ptr.is_null() {
            return;
        }
        let layout = Layout::from_size_align(
            self.cap * core::mem::size_of::<T>(),
            core::mem::align_of::<T>(),
        )
        .expect("RawVec layout invalid at drop");
        unsafe { crate::alloc::GLOBAL.dealloc(self.ptr as *mut u8, layout) };
    }
}
