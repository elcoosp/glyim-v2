//! Operator traits and range types for the Glyim core library.

// === Dereference traits ===

/// Used for immutable dereferencing operations, like `*v`.
trait Deref {
    type Target;
    fn deref(&self) -> &Self::Target;
}

/// Used for mutable dereferencing operations, like `*v = 1;`.
trait DerefMut: Deref {
    fn deref_mut(&mut self) -> &mut Self::Target;
}

/// Code that is run when a value is dropped.
trait Drop {
    fn drop(&mut self);
}

// === Function traits ===

/// The version of the call operator that takes a by-value receiver.
trait FnOnce<Args> {
    type Output;
    fn call_once(self, args: Args) -> Self::Output;
}

/// The version of the call operator that takes a mutable receiver.
trait FnMut<Args>: FnOnce<Args> {
    fn call_mut(&mut self, args: Args) -> Self::Output;
}

/// The version of the call operator that takes an immutable receiver.
trait Fn<Args>: FnMut<Args> {
    fn call(&self, args: Args) -> Self::Output;
}

// === Arithmetic traits ===

/// The addition operator `+`.
trait Add<Rhs = Self> {
    type Output;
    fn add(self, rhs: Rhs) -> Self::Output;
}

/// The subtraction operator `-`.
trait Sub<Rhs = Self> {
    type Output;
    fn sub(self, rhs: Rhs) -> Self::Output;
}

/// The multiplication operator `*`.
trait Mul<Rhs = Self> {
    type Output;
    fn mul(self, rhs: Rhs) -> Self::Output;
}

/// The division operator `/`.
trait Div<Rhs = Self> {
    type Output;
    fn div(self, rhs: Rhs) -> Self::Output;
}

/// The remainder operator `%`.
trait Rem<Rhs = Self> {
    type Output;
    fn rem(self, rhs: Rhs) -> Self::Output;
}

/// The unary negation operator `-`.
trait Neg {
    type Output;
    fn neg(self) -> Self::Output;
}

/// The unary logical negation operator `!`.
trait Not {
    type Output;
    fn not(self) -> Self::Output;
}

/// The bitwise AND operator `&`.
trait BitAnd<Rhs = Self> {
    type Output;
    fn bitand(self, rhs: Rhs) -> Self::Output;
}

/// The bitwise OR operator `|`.
trait BitOr<Rhs = Self> {
    type Output;
    fn bitor(self, rhs: Rhs) -> Self::Output;
}

/// The bitwise XOR operator `^`.
trait BitXor<Rhs = Self> {
    type Output;
    fn bitxor(self, rhs: Rhs) -> Self::Output;
}

/// The left shift operator `<<`.
trait Shl<Rhs = Self> {
    type Output;
    fn shl(self, rhs: Rhs) -> Self::Output;
}

/// The right shift operator `>>`.
trait Shr<Rhs = Self> {
    type Output;
    fn shr(self, rhs: Rhs) -> Self::Output;
}

// === Compound assignment traits ===

/// The addition assignment operator `+=`.
trait AddAssign<Rhs = Self> {
    fn add_assign(&mut self, rhs: Rhs);
}

/// The subtraction assignment operator `-=`.
trait SubAssign<Rhs = Self> {
    fn sub_assign(&mut self, rhs: Rhs);
}

/// The multiplication assignment operator `*=`.
trait MulAssign<Rhs = Self> {
    fn mul_assign(&mut self, rhs: Rhs);
}

/// The division assignment operator `/=`.
trait DivAssign<Rhs = Self> {
    fn div_assign(&mut self, rhs: Rhs);
}

// === Index traits ===

/// Used for indexing operations (`container[index]`).
trait Index<Idx> {
    type Output;
    fn index(&self, index: Idx) -> &Self::Output;
}

/// Used for mutable indexing operations (`container[index]`).
trait IndexMut<Idx>: Index<Idx> {
    fn index_mut(&mut self, index: Idx) -> &mut Self::Output;
}

// === Range types ===

/// An unbounded range (`..`).
struct RangeFull;

/// A (half-open) range bounded inclusively below and exclusively above (`start..end`).
struct Range<Idx> {
    pub start: Idx,
    pub end: Idx,
}

/// A range bounded inclusively below and above (`start..=end`).
struct RangeInclusive<Idx> {
    pub start: Idx,
    pub end: Idx,
}

/// A range bounded inclusively below and unbounded above (`start..`).
struct RangeFrom<Idx> {
    pub start: Idx,
}

/// A range bounded exclusively above and unbounded below (`..end`).
struct RangeTo<Idx> {
    pub end: Idx,
}

/// A range bounded exclusively below and inclusively above (`..=end`).
struct RangeToInclusive<Idx> {
    pub end: Idx,
}
