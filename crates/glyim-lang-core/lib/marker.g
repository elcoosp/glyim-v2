//! Marker traits and types for the Glyim core library.

/// Types with a constant size known at compile time.
trait Sized {}

/// Types that can be copied by simply copying bits (memcpy).
trait Copy: Clone {}

/// Types that can be safely sent across thread boundaries.
trait Send: Sized {}

/// Types that can be safely shared between threads.
trait Sync: Sized {}

/// A type that can be used to "mark" another type as owning `T`
/// without actually storing a value of type `T`.
struct PhantomData<T> {}

impl<T> Clone for PhantomData<T> {
    fn clone(&self) -> Self { PhantomData {} }
}

impl<T> Copy for PhantomData<T> {}

impl<T> Default for PhantomData<T> {
    fn default() -> Self { PhantomData {} }
}

/// A marker type which does not implement `Unpin`.
struct PhantomPinned {}

impl Clone for PhantomPinned {
    fn clone(&self) -> Self { PhantomPinned {} }
}

impl Copy for PhantomPinned {}
