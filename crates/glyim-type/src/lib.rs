//! Core type system & Type Context.
pub mod ty;
pub mod region;
pub mod substitution;
pub mod binder;
pub mod flags;
pub mod display;
pub mod const_val;
pub mod fn_sig;
pub mod context;

pub use ty::*;
pub use region::BoundRegionKind;
pub use binder::{Binder, BoundVariableKind};
pub use const_val::{Const, ConstKind};
pub use fn_sig::FnSig;
pub use flags::TypeFlags;
pub use display::TypeLookup;
pub use context::{TyCtxMut, TyCtx};
pub use display::PrintTy;
