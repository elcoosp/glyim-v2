//! A UTF-8 encoded, growable string for the Glyim alloc library.

use vec::Vec;
use default::Default;

/// A mutable string type backed by a `Vec<u8>`.
struct String {
    inner: Vec<u8>,
}

impl String {
    /// Creates a new empty `String`.
    fn new() -> Self {
        String { inner: Vec::new() }
    }

    /// Appends a string slice to the end.
    fn push_str(&mut self, s: &str) {
        self.inner.extend_from_slice(s.as_bytes());
    }

    /// Returns the string as a `&str`.
    fn as_str(&self) -> &str {
        str::from_utf8(self.inner.as_slice()).expect("String contains invalid UTF-8")
    }

    /// Returns the length in bytes.
    fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the string is empty.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for String {
    fn default() -> Self {
        Self::new()
    }
}
