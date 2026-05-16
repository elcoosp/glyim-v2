//! Heap-allocated owned pointer `Box<T>` for the Glyim alloc library.

use alloc::{GlobalAlloc, Layout};

/// A pointer type that uniquely owns a heap allocation of type `T`.
struct Box<T> {
    ptr: *mut T,
}

impl<T> Box<T> {
    /// Allocate memory on the heap and move `value` into it.
    fn new(value: T) -> Self {
        let layout = Layout::from_size_align(
            mem::size_of::<T>(),
            mem::align_of::<T>(),
        ).expect("Box layout invalid");
        let ptr = GLOBAL.alloc(layout) as *mut T;
        if ptr.is_null() {
            handle_alloc_error(layout);
        }
        ptr::write(ptr, value);
        Box { ptr }
    }
}

impl<T> Deref for Box<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for Box<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

impl<T> Drop for Box<T> {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(
            mem::size_of_val(self),
            mem::align_of_val(self),
        ).expect("Box layout invalid at drop");
        unsafe {
            ptr::drop_in_place(self.ptr);
            GLOBAL.dealloc(self.ptr as *mut u8, layout);
        }
    }
}
