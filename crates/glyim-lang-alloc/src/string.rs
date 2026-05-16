//! A UTF-8 encoded, growable string.
//! Stub – full implementation in progress.

use crate::vec::Vec;

/// A mutable string type backed by a `Vec<u8>`.
pub struct String {
    inner: Vec<u8>,
}

impl String {
    /// Creates a new empty `String`.
    pub const fn new() -> Self {
        String { inner: Vec::new() }
    }

    /// Appends a string slice to the end.
    pub fn push_str(&mut self, s: &str) {
        self.inner.extend_from_slice(s.as_bytes());
    }

    /// Returns the string as a `&str`.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.inner.as_slice()).expect("String contains invalid UTF-8")
    }

    /// Returns the length in bytes.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the string is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}
