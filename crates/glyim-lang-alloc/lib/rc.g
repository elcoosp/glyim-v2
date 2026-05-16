//! Reference-counted pointer `Rc<T>` for the Glyim alloc library.

use alloc::{GlobalAlloc, Layout};
use cell::Cell;
use ops::Deref;
use clone::Clone;

struct RcInner<T> {
    strong: Cell<usize>,
    value: T,
}

/// A single-threaded reference-counting pointer.
struct Rc<T> {
    ptr: *mut RcInner<T>,
}

impl<T> Rc<T> {
    /// Constructs a new `Rc<T>`.
    fn new(value: T) -> Self {
        let layout = Layout::from_size_align(
            mem::size_of::<RcInner<T>>(),
            mem::align_of::<RcInner<T>>(),
        ).expect("Rc layout invalid");
        let ptr = GLOBAL.alloc(layout) as *mut RcInner<T>;
        if ptr.is_null() {
            handle_alloc_error(layout);
        }
        unsafe {
            ptr::write(ptr, RcInner {
                strong: Cell::new(1),
                value,
            });
        }
        Rc { ptr }
    }

    /// Returns the number of strong references.
    fn strong_count(this: &Self) -> usize {
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
                ptr::drop_in_place(self.ptr);
                let layout = Layout::from_size_align(
                    mem::size_of::<RcInner<T>>(),
                    mem::align_of::<RcInner<T>>(),
                ).expect("Rc layout invalid at drop");
                GLOBAL.dealloc(self.ptr as *mut u8, layout);
            }
        }
    }
}
