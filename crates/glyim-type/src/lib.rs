//! Core type system & Type Context.
pub mod adt_def;
pub mod auto_trait;
pub mod binder;
pub mod const_val;
pub mod display;
pub mod flags;
pub mod fn_sig;
pub mod predicate;
pub mod region;
pub mod substitution;
pub mod ty;
pub mod ty_ctx;
pub mod ty_ctx_mut;

pub use adt_def::*;
pub use auto_trait::*;
pub use binder::*;
pub use const_val::*;
pub use display::*;
pub use flags::*;
pub use fn_sig::*;
pub use predicate::*;
pub use region::*;
pub use substitution::*;
pub use ty::*;
pub use ty_ctx::*;
pub use ty_ctx_mut::*;

pub mod object_safety;

#[cfg(test)]
mod tests;
