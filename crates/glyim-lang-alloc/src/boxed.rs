//! Heap-allocated owned pointer `Box<T>`.
//! Stub – full implementation in progress.

use crate::alloc::{GlobalAlloc, Layout};
use core::ops::{Deref, DerefMut};

/// A pointer type that uniquely owns a heap allocation of type `T`.
pub struct Box<T: ?Sized> {
    ptr: *mut T,
}

impl<T> Box<T> {
    /// Allocate memory on the heap and move `value` into it.
    pub fn new(value: T) -> Self {
        let layout = Layout::from_size_align(core::mem::size_of::<T>(), core::mem::align_of::<T>())
            .expect("Box layout invalid");
        // SAFETY: we allocate with the global allocator; stub does not actually call alloc.
        // Full implementation will integrate the global allocator.
        let ptr = crate::alloc::GLOBAL.alloc(layout) as *mut T;
        if ptr.is_null() {
            crate::alloc::handle_alloc_error(layout);
        }
        unsafe { core::ptr::write(ptr, value) };
        Box { ptr }
    }
}

impl<T: ?Sized> Drop for Box<T> {
    fn drop(&mut self) {
        // SAFETY: ptr was allocated by Box::new with the same layout.
        let layout = Layout::from_size_align(core::mem::size_of_val(unsafe { &*self.ptr }),
                                             core::mem::align_of_val(unsafe { &*self.ptr }))
            .expect("Box layout invalid at drop");
        unsafe {
            core::ptr::drop_in_place(self.ptr);
            crate::alloc::GLOBAL.dealloc(self.ptr as *mut u8, layout);
        }
    }
}

impl<T: ?Sized> Deref for Box<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T: ?Sized> DerefMut for Box<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}
