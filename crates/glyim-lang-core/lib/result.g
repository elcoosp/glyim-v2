//! The `Result` type for the Glyim core library.
//!
//! `Result<T, E>` is the type used for returning and propagating errors.
//! It is an enum with the variants `Ok(T)`, representing success and containing
//! a value, and `Err(E)`, representing error and containing an error value.

/// `Result` is a type that represents either success (`Ok`) or failure (`Err`).
enum Result<T, E> {
    /// Contains the success value.
    Ok(T),
    /// Contains the error value.
    Err(E),
}

impl<T, E> Result<T, E> {
    /// Returns `true` if the result is `Ok`.
    fn is_ok(&self) -> bool {
        match self {
            Result::Ok(_) => true,
            Result::Err(_) => false,
        }
    }

    /// Returns `true` if the result is `Err`.
    fn is_err(&self) -> bool {
        match self {
            Result::Ok(_) => false,
            Result::Err(_) => true,
        }
    }

    /// Converts `self` into an `Option<T>`, consuming `self`, and discarding the error, if any.
    fn ok(self) -> Option<T> {
        match self {
            Result::Ok(val) => Option::Some(val),
            Result::Err(_) => Option::None,
        }
    }

    /// Converts `self` into an `Option<E>`, consuming `self`, and discarding the success value, if any.
    fn err(self) -> Option<E> {
        match self {
            Result::Ok(_) => Option::None,
            Result::Err(err) => Option::Some(err),
        }
    }

    /// Returns the contained `Ok` value or a provided default.
    fn unwrap_or(self, default: T) -> T {
        match self {
            Result::Ok(val) => val,
            Result::Err(_) => default,
        }
    }

    /// Returns the contained `Ok` value or computes it from a closure.
    fn unwrap_or_else(self, op: fn(E) -> T) -> T {
        match self {
            Result::Ok(val) => val,
            Result::Err(err) => op(err),
        }
    }

    /// Returns the contained `Ok` value or a default.
    fn unwrap_or_default(self) -> T where T: Default {
        match self {
            Result::Ok(val) => val,
            Result::Err(_) => T::default(),
        }
    }

    /// Returns the contained `Ok` value, consuming the `self` value.
    ///
    /// Panics if the value is an `Err`, with a panic message including the
    /// passed message.
    fn expect(self, msg: &str) -> T {
        match self {
            Result::Ok(val) => val,
            Result::Err(_) => panic!("{}", msg),
        }
    }

    /// Returns the contained `Ok` value, consuming the `self` value.
    ///
    /// Panics if the value is an `Err`.
    fn unwrap(self) -> T {
        match self {
            Result::Ok(val) => val,
            Result::Err(_) => panic!("called `Result::unwrap()` on an `Err` value"),
        }
    }

    /// Returns the contained `Err` value, consuming the `self` value.
    ///
    /// Panics if the value is an `Ok`.
    fn expect_err(self, msg: &str) -> E {
        match self {
            Result::Ok(_) => panic!("{}", msg),
            Result::Err(err) => err,
        }
    }

    /// Returns the contained `Err` value, consuming the `self` value.
    fn unwrap_err(self) -> E {
        match self {
            Result::Ok(_) => panic!("called `Result::unwrap_err()` on an `Ok` value"),
            Result::Err(err) => err,
        }
    }

    /// Maps a `Result<T, E>` to `Result<U, E>` by applying a function to a
    /// contained `Ok` value, leaving an `Err` value untouched.
    fn map<U>(self, op: fn(T) -> U) -> Result<U, E> {
        match self {
            Result::Ok(val) => Result::Ok(op(val)),
            Result::Err(err) => Result::Err(err),
        }
    }

    /// Maps a `Result<T, E>` to `Result<T, F>` by applying a function to a
    /// contained `Err` value, leaving an `Ok` value untouched.
    fn map_err<F>(self, op: fn(E) -> F) -> Result<T, F> {
        match self {
            Result::Ok(val) => Result::Ok(val),
            Result::Err(err) => Result::Err(op(err)),
        }
    }

    /// Returns `res` if the result is `Ok`, otherwise returns the `Err` value of `self`.
    fn and<U>(self, res: Result<U, E>) -> Result<U, E> {
        match self {
            Result::Ok(_) => res,
            Result::Err(err) => Result::Err(err),
        }
    }

    /// Calls `op` if the result is `Ok`, otherwise returns the `Err` value of `self`.
    fn and_then<U>(self, op: fn(T) -> Result<U, E>) -> Result<U, E> {
        match self {
            Result::Ok(val) => op(val),
            Result::Err(err) => Result::Err(err),
        }
    }

    /// Returns `res` if the result is `Err`, otherwise returns the `Ok` value of `self`.
    fn or<F>(self, res: Result<T, F>) -> Result<T, F> {
        match self {
            Result::Ok(val) => Result::Ok(val),
            Result::Err(_) => res,
        }
    }

    /// Calls `op` if the result is `Err`, otherwise returns the `Ok` value of `self`.
    fn or_else<F>(self, op: fn(E) -> Result<T, F>) -> Result<T, F> {
        match self {
            Result::Ok(val) => Result::Ok(val),
            Result::Err(err) => op(err),
        }
    }

    /// Converts from `&Result<T, E>` to `Result<&T, &E>`.
    fn as_ref(&self) -> Result<&T, &E> {
        match self {
            Result::Ok(val) => Result::Ok(val),
            Result::Err(err) => Result::Err(err),
        }
    }

    /// Converts from `&mut Result<T, E>` to `Result<&mut T, &mut E>`.
    fn as_mut(&mut self) -> Result<&mut T, &mut E> {
        match self {
            Result::Ok(val) => Result::Ok(val),
            Result::Err(err) => Result::Err(err),
        }
    }
}
