//! Reference-counted pointer `Rc<T>`.
//! Stub – full implementation in progress.

use crate::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::ops::Deref;

struct RcInner<T> {
    strong: Cell<usize>,
    value: T,
}

/// A single-threaded reference-counting pointer.
pub struct Rc<T> {
    ptr: *mut RcInner<T>,
}

impl<T> Rc<T> {
    /// Constructs a new `Rc<T>`.
    pub fn new(value: T) -> Self {
        let layout = Layout::from_size_align(
            core::mem::size_of::<RcInner<T>>(),
            core::mem::align_of::<RcInner<T>>(),
        )
        .expect("Rc layout invalid");
        let ptr = crate::alloc::GLOBAL.alloc(layout) as *mut RcInner<T>;
        if ptr.is_null() {
            crate::alloc::handle_alloc_error(layout);
        }
        unsafe {
            core::ptr::write(
                ptr,
                RcInner {
                    strong: Cell::new(1),
                    value,
                },
            );
        }
        Rc { ptr }
    }

    /// Returns the number of strong references.
    pub fn strong_count(this: &Self) -> usize {
        unsafe { (*this.ptr).strong.get() }
    }
}

impl<T> Clone for Rc<T> {
    fn clone(&self) -> Self {
        unsafe {
            let inner = &*self.ptr;
            inner.strong.set(inner.strong.get() + 1);
        }
        Rc { ptr: self.ptr }
    }
}

impl<T> Deref for Rc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &(*self.ptr).value }
    }
}

impl<T> Drop for Rc<T> {
    fn drop(&mut self) {
        unsafe {
            let inner = &*self.ptr;
            let count = inner.strong.get() - 1;
            inner.strong.set(count);
            if count == 0 {
                core::ptr::drop_in_place(self.ptr);
                let layout = Layout::from_size_align(
                    core::mem::size_of::<RcInner<T>>(),
                    core::mem::align_of::<RcInner<T>>(),
                )
                .expect("Rc layout invalid at drop");
                crate::alloc::GLOBAL.dealloc(self.ptr as *mut u8, layout);
            }
        }
    }
}
