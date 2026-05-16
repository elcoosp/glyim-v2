//! The `Default` trait for the Glyim core library.

/// A trait for giving a type a useful default value.
trait Default: Sized {
    /// Returns the "default value" for a type.
    fn default() -> Self;
}

impl Default for () { fn default() -> Self {} }
impl Default for bool { fn default() -> Self { false } }
impl Default for u8 { fn default() -> Self { 0 } }
impl Default for u16 { fn default() -> Self { 0 } }
impl Default for u32 { fn default() -> Self { 0 } }
impl Default for u64 { fn default() -> Self { 0 } }
impl Default for usize { fn default() -> Self { 0 } }
impl Default for i8 { fn default() -> Self { 0 } }
impl Default for i16 { fn default() -> Self { 0 } }
impl Default for i32 { fn default() -> Self { 0 } }
impl Default for i64 { fn default() -> Self { 0 } }
impl Default for isize { fn default() -> Self { 0 } }
impl Default for f32 { fn default() -> Self { 0.0 } }
impl Default for f64 { fn default() -> Self { 0.0 } }
impl Default for char { fn default() -> Self { '\0' } }

impl<T: Default> Default for Option<T> {
    fn default() -> Self { Option::None }
}
