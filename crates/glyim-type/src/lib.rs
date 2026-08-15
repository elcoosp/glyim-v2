//! Core type system & Type Context.
#![allow(missing_docs)]
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

/// Definition of a trait for the type context.
#[derive(Clone, Debug)]
pub struct TraitDef {
    pub name: glyim_core::interner::Name,
    pub methods: Vec<MethodDef>,
}

/// Definition of a method in a trait.
#[derive(Clone, Debug)]
pub struct MethodDef {
    pub name: glyim_core::interner::Name,
    pub sig: FnSig,
    /// The `FnDefId` of the (canonical, trait-level) method definition.
    /// When a vtable is generated for a concrete impl, this identifies the
    /// method whose monomorphized body should be dispatched. `None` when the
    /// method's def id is not yet known (e.g. built-in traits).
    pub fn_def_id: Option<glyim_core::def_id::FnDefId>,
}
