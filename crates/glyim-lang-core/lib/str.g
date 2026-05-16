//! String slice operations for the Glyim core library.

/// Extension methods for string slices.
impl str {
    /// Returns the length of `self`.
    fn len(&self) -> usize {
        // compiler intrinsic
    }

    /// Returns `true` if `self` has a length of zero bytes.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` if the given pattern matches a sub-slice of this string slice.
    fn contains(&self, needle: &str) -> bool {
        // compiler intrinsic or pattern matching
    }

    /// Returns an iterator over the chars of the string slice.
    fn chars(&self) -> Chars {
        Chars { s: self }
    }

    /// Returns an iterator over the lines of the string.
    fn lines(&self) -> Lines {
        Lines { s: self }
    }

    /// Returns a string slice with leading and trailing whitespace removed.
    fn trim(&self) -> &str {
        // compiler intrinsic
    }

    /// Parses this string slice into another type.
    fn parse<T: FromStr>(&self) -> Result<T, T::Err> {
        T::from_str(self)
    }
}

/// An iterator over the chars of a string slice.
struct Chars<'a> {
    s: &'a str,
}

impl<'a> Iterator for Chars<'a> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        // compiler intrinsic - yields next unicode scalar
    }
}

/// An iterator over the lines of a string.
struct Lines<'a> {
    s: &'a str,
}

impl<'a> Iterator for Lines<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        // yields next line (split on \n, stripping \r)
    }
}

/// A trait for types that can be parsed from a string.
trait FromStr: Sized {
    /// The associated error which can be returned from parsing.
    type Err;

    /// Parses a string `s` to return a value of this type.
    fn from_str(s: &str) -> Result<Self, Self::Err>;
}
