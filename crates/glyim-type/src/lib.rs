//! Core type system & Type Context.
pub mod binder;
pub mod const_val;
pub mod context;
pub mod display;
pub mod flags;
pub mod fn_sig;
pub mod predicate;
pub mod region;
pub mod substitution;
pub mod ty;

pub use binder::*;
pub use const_val::*;
pub use context::*;
pub use display::*;
pub use flags::*;
pub use fn_sig::*;
pub use predicate::*;
pub use region::*;
pub use substitution::*;
pub use ty::*;

#[cfg(test)]
mod tests;
