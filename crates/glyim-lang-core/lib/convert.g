//! Type conversion traits for the Glyim core library.

/// The `Into` trait for consuming conversion.
trait Into<T>: Sized {
    /// Performs the conversion.
    fn into(self) -> T;
}

/// The `From` trait for constructing a value from another type.
trait From<T>: Sized {
    /// Performs the conversion.
    fn from(value: T) -> Self;
}

/// Blanket implementation: any type `T` can be converted `Into` itself.
impl<T> Into<T> for T {
    fn into(self) -> T { self }
}

/// A fallible version of `From` that can return an error.
trait TryFrom<T>: Sized {
    /// The type returned in the event of a conversion error.
    type Error;
    /// Performs the conversion.
    fn try_from(value: T) -> Result<Self, Self::Error>;
}

/// A fallible version of `Into` that can return an error.
trait TryInto<T>: Sized {
    /// The type returned in the event of a conversion error.
    type Error;
    /// Performs the conversion.
    fn try_into(self) -> Result<T, Self::Error>;
}

impl<T, U: TryFrom<T>> TryInto<U> for T {
    type Error = U::Error;
    fn try_into(self) -> Result<U, Self::Error> {
        U::try_from(self)
    }
}

/// The `AsRef` trait for cheap reference-to-reference conversion.
trait AsRef<T> {
    /// Performs the conversion.
    fn as_ref(&self) -> &T;
}

/// The `AsMut` trait for cheap mutable reference conversion.
trait AsMut<T> {
    /// Performs the conversion.
    fn as_mut(&mut self) -> &mut T;
}

/// An uninhabited type that can never be constructed.
enum Infallible {}
