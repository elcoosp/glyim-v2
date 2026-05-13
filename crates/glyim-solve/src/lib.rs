//! Custom trait solver and type inference engine.
//!
//! [F3+F14] Does NOT define its own `PrintTy`. Imports
//! `glyim_type::PrintTy` which is generic over `TypeLookup`.
//!
//! [F18] Separate `IndexVec`s for `TyVar`, `IntVar`, and
//! `FloatVar`. The type system prevents constructing
//! `InferVar::Int(TyVar(0))` because `Int` takes an `IntVar`.

pub mod infer;
pub mod fulfill;
pub mod solver;

pub use infer::*;
pub use fulfill::*;
pub use solver::*;
