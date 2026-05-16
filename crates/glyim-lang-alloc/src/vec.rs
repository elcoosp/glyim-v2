//! A contiguous growable array type, `Vec<T>`.
//! Stub – full implementation in progress.

use crate::raw_vec::RawVec;
use core::ops::{Index, IndexMut};

/// A growable array.
pub struct Vec<T> {
    buf: RawVec<T>,
    len: usize,
}

impl<T> Vec<T> {
    /// Creates a new empty `Vec`.
    pub const fn new() -> Self {
        Vec {
            buf: RawVec::new(),
            len: 0,
        }
    }

    /// Appends an element to the back.
    pub fn push(&mut self, value: T) {
        if self.len == self.buf.capacity() {
            self.buf.reserve(self.len + 1);
        }
        unsafe {
            let end = self.buf.as_mut_ptr().add(self.len);
            core::ptr::write(end, value);
        }
        self.len += 1;
    }

    /// Removes the last element and returns it, or `None` if empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe { Some(core::ptr::read(self.buf.as_ptr().add(self.len))) }
        }
    }

    /// Returns the number of elements.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the vector contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Extracts a slice containing the entire vector.
    pub fn as_slice(&self) -> &[T] {
        if self.len == 0 {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(self.buf.as_ptr(), self.len) }
        }
    }

    /// Extends the vector with elements from a slice.
    pub fn extend_from_slice(&mut self, other: &[T]) {
        self.buf.reserve(self.len + other.len());
        for item in other {
            unsafe {
                let end = self.buf.as_mut_ptr().add(self.len);
                core::ptr::write(end, core::ptr::read(item));
            }
            self.len += 1;
        }
    }
}

impl<T> Index<usize> for Vec<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        assert!(index < self.len, "Vec index out of bounds");
        unsafe { &*self.buf.as_ptr().add(index) }
    }
}

impl<T> IndexMut<usize> for Vec<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        assert!(index < self.len, "Vec index out of bounds");
        unsafe { &mut *self.buf.as_mut_ptr().add(index) }
    }
}
