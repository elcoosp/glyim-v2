//! Runtime values produced by const evaluation.

use glyim_core::interner::Name;
use glyim_core::primitives::{FloatTy, IntTy, UintTy};

/// A value produced by constant evaluation.
///
/// This represents the result of evaluating a constant expression
/// at compile time. It covers all literal types supported by the
/// HIR `Literal` enum.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    /// Signed integer value with its specific type.
    Int(i128, IntTy),
    /// Unsigned integer value with its specific type.
    Uint(u128, UintTy),
    /// Float value stored as bits with its specific type.
    FloatBits(u64, FloatTy),
    /// Boolean value.
    Bool(bool),
    /// Character value.
    Char(char),
    /// String value (interned name).
    String(Name),
    /// Unit value `()`.
    Unit,
    /// Tuple value.
    Tuple(Vec<ConstValue>),
}

impl ConstValue {
    /// Validate that this value fits within the range of its declared type.
    ///
    /// After arithmetic on `i128`/`u128` representations, the result may
    /// exceed the range of the specific integer type (e.g., `i32`). This
    /// method checks the range and returns `None` if the value overflows.
    pub fn validate_range(&self) -> Option<ConstValue> {
        match self {
            ConstValue::Int(v, IntTy::I8) => {
                if *v >= i8::MIN as i128 && *v <= i8::MAX as i128 {
                    Some(self.clone())
                } else {
                    None
                }
            }
            ConstValue::Int(v, IntTy::I16) => {
                if *v >= i16::MIN as i128 && *v <= i16::MAX as i128 {
                    Some(self.clone())
                } else {
                    None
                }
            }
            ConstValue::Int(v, IntTy::I32) => {
                if *v >= i32::MIN as i128 && *v <= i32::MAX as i128 {
                    Some(self.clone())
                } else {
                    None
                }
            }
            ConstValue::Int(v, IntTy::I64) => {
                if *v >= i64::MIN as i128 && *v <= i64::MAX as i128 {
                    Some(self.clone())
                } else {
                    None
                }
            }
            ConstValue::Int(v, IntTy::Isize) => {
                // Assume 64-bit target for const eval
                if *v >= i64::MIN as i128 && *v <= i64::MAX as i128 {
                    Some(self.clone())
                } else {
                    None
                }
            }
            ConstValue::Uint(v, UintTy::U8) => {
                if *v <= u8::MAX as u128 {
                    Some(self.clone())
                } else {
                    None
                }
            }
            ConstValue::Uint(v, UintTy::U16) => {
                if *v <= u16::MAX as u128 {
                    Some(self.clone())
                } else {
                    None
                }
            }
            ConstValue::Uint(v, UintTy::U32) => {
                if *v <= u32::MAX as u128 {
                    Some(self.clone())
                } else {
                    None
                }
            }
            ConstValue::Uint(v, UintTy::U64) => {
                if *v <= u64::MAX as u128 {
                    Some(self.clone())
                } else {
                    None
                }
            }
            ConstValue::Uint(v, UintTy::Usize) => {
                // Assume 64-bit target for const eval
                if *v <= u64::MAX as u128 {
                    Some(self.clone())
                } else {
                    None
                }
            }
            // Float, Bool, Char, String, Unit, Tuple don't overflow
            _ => Some(self.clone()),
        }
    }

    /// Returns `true` if this value is a boolean.
    pub fn is_bool(&self) -> bool {
        matches!(self, ConstValue::Bool(_))
    }

    /// Returns the boolean value if this is a `Bool`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConstValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Returns the signed integer value if this is an `Int`.
    pub fn as_i128(&self) -> Option<i128> {
        match self {
            ConstValue::Int(v, _) => Some(*v),
            _ => None,
        }
    }

    /// Returns the unsigned integer value if this is a `Uint`.
    pub fn as_u128(&self) -> Option<u128> {
        match self {
            ConstValue::Uint(v, _) => Some(*v),
            _ => None,
        }
    }

    /// Attempt to add two const values, returning `None` if they are
    /// incompatible types.
    pub fn checked_add(&self, other: &ConstValue) -> Option<ConstValue> {
        match (self, other) {
            (ConstValue::Int(a, ty_a), ConstValue::Int(b, ty_b)) if ty_a == ty_b => {
                a.checked_add(*b).map(|v| ConstValue::Int(v, *ty_a))
            }
            (ConstValue::Uint(a, ty_a), ConstValue::Uint(b, ty_b)) if ty_a == ty_b => {
                a.checked_add(*b).map(|v| ConstValue::Uint(v, *ty_a))
            }
            _ => None,
        }
    }

    /// Attempt to subtract two const values.
    pub fn checked_sub(&self, other: &ConstValue) -> Option<ConstValue> {
        match (self, other) {
            (ConstValue::Int(a, ty_a), ConstValue::Int(b, ty_b)) if ty_a == ty_b => {
                a.checked_sub(*b).map(|v| ConstValue::Int(v, *ty_a))
            }
            (ConstValue::Uint(a, ty_a), ConstValue::Uint(b, ty_b)) if ty_a == ty_b => {
                a.checked_sub(*b).map(|v| ConstValue::Uint(v, *ty_a))
            }
            _ => None,
        }
    }

    /// Attempt to multiply two const values.
    pub fn checked_mul(&self, other: &ConstValue) -> Option<ConstValue> {
        match (self, other) {
            (ConstValue::Int(a, ty_a), ConstValue::Int(b, ty_b)) if ty_a == ty_b => {
                a.checked_mul(*b).map(|v| ConstValue::Int(v, *ty_a))
            }
            (ConstValue::Uint(a, ty_a), ConstValue::Uint(b, ty_b)) if ty_a == ty_b => {
                a.checked_mul(*b).map(|v| ConstValue::Uint(v, *ty_a))
            }
            _ => None,
        }
    }

    /// Attempt to divide two const values.
    pub fn checked_div(&self, other: &ConstValue) -> Option<ConstValue> {
        match (self, other) {
            (ConstValue::Int(a, ty_a), ConstValue::Int(b, ty_b)) if ty_a == ty_b => {
                a.checked_div(*b).map(|v| ConstValue::Int(v, *ty_a))
            }
            (ConstValue::Uint(a, ty_a), ConstValue::Uint(b, ty_b)) if ty_a == ty_b => {
                a.checked_div(*b).map(|v| ConstValue::Uint(v, *ty_a))
            }
            _ => None,
        }
    }

    /// Attempt to compute remainder of two const values.
    pub fn checked_rem(&self, other: &ConstValue) -> Option<ConstValue> {
        match (self, other) {
            (ConstValue::Int(a, ty_a), ConstValue::Int(b, ty_b)) if ty_a == ty_b => {
                a.checked_rem(*b).map(|v| ConstValue::Int(v, *ty_a))
            }
            (ConstValue::Uint(a, ty_a), ConstValue::Uint(b, ty_b)) if ty_a == ty_b => {
                a.checked_rem(*b).map(|v| ConstValue::Uint(v, *ty_a))
            }
            _ => None,
        }
    }

    /// Perform a negation on this value.
    pub fn checked_neg(&self) -> Option<ConstValue> {
        match self {
            ConstValue::Int(v, ty) => v.checked_neg().map(|r| ConstValue::Int(r, *ty)),
            _ => None,
        }
    }

    /// Perform a logical or bitwise not on this value.
    pub fn not(&self) -> Option<ConstValue> {
        match self {
            ConstValue::Bool(b) => Some(ConstValue::Bool(!b)),
            ConstValue::Int(v, ty) => Some(ConstValue::Int(!v, *ty)),
            ConstValue::Uint(v, ty) => Some(ConstValue::Uint(!v, *ty)),
            _ => None,
        }
    }
}
