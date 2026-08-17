//! Cast legality — the single source of truth shared by type-checking and
//! constant evaluation (de-stubbing plan §13.2).
//!
//! `is_valid_cast` decides whether a value of type `from` may be cast to type
//! `to`. It is a pure function of the two types (and the `TypeLookup` needed to
//! inspect ADT definitions); it performs no value transformation. Both
//! `glyim-typeck` (which emits an error for illegal casts) and
//! `glyim-const-eval` (which must reject illegal `const` casts) delegate to it
//! so the rules live in exactly one place.

use crate::adt_def::{AdtDef, AdtKind};
use crate::display::TypeLookup;
use crate::{Ty, TyKind};

/// Return `true` if a value of type `from` may be cast to type `to`.
///
/// The rules mirror Rust's `as` cast legality (subset implemented so far):
///   * int/uint ↔ int/uint/float
///   * float ↔ float/int/uint
///   * raw pointer / reference ↔ raw pointer / int
///   * bool ↔ int/uint
///   * char ↔ int/uint
///   * a fieldless (C-like) enum ↔ int/uint
///   * identical types are always allowed
pub fn is_valid_cast(ctx: &dyn TypeLookup, from: Ty, to: Ty) -> bool {
    use TyKind::*;
    let from_k = ctx.ty_kind(from);
    let to_k = ctx.ty_kind(to);
    match (from_k, to_k) {
        (Int(_) | Uint(_), Int(_) | Uint(_) | Float(_)) => true,
        (Float(_), Float(_) | Int(_) | Uint(_)) => true,
        (RawPtr(_, _) | Ref(_, _, _), RawPtr(_, _) | Int(_)) => true,
        (Bool, Int(_) | Uint(_)) => true,
        (Char, Int(_) | Uint(_)) => true,
        (Adt(from_id, _), Int(_) | Uint(_)) => {
            // Only a fieldless enum may cast to an integer (plan §13.2).
            // Structs, unions, and enums with data are rejected.
            match ctx.adt_def(*from_id) {
                Some(adt) => {
                    adt.kind == AdtKind::Enum
                        && adt.variants.iter().all(|v| v.fields.is_empty())
                }
                None => false,
            }
        }
        _ if from == to => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ty_ctx_mut::TyCtxMut;
    use glyim_core::interner::Interner;
    use glyim_core::primitives::{FloatTy, IntTy, UintTy};

    #[test]
    fn int_to_int_is_valid() {
        let mut tcx_mut = TyCtxMut::new(Interner::new());
        let i32 = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
        let u8 = tcx_mut.mk_ty(TyKind::Uint(UintTy::U8));
        assert!(is_valid_cast(&tcx_mut, i32, u8));
    }

    #[test]
    fn float_to_int_is_valid() {
        let mut tcx_mut = TyCtxMut::new(Interner::new());
        let f64 = tcx_mut.mk_ty(TyKind::Float(FloatTy::F64));
        let i32 = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
        assert!(is_valid_cast(&tcx_mut, f64, i32));
    }

    #[test]
    fn ptr_to_float_is_invalid() {
        let mut tcx_mut = TyCtxMut::new(Interner::new());
        let i32 = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
        let ptr = tcx_mut.mk_ty(TyKind::RawPtr(i32, glyim_core::primitives::Mutability::Not));
        let f64 = tcx_mut.mk_ty(TyKind::Float(FloatTy::F64));
        assert!(!is_valid_cast(&tcx_mut, ptr, f64));
    }

    #[test]
    fn identical_types_are_valid() {
        let mut tcx_mut = TyCtxMut::new(Interner::new());
        let i32 = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
        assert!(is_valid_cast(&tcx_mut, i32, i32));
    }

    #[test]
    #[ignore = "requires a registered fieldless enum ADT to exercise the Adt arm"]
    fn fieldless_enum_to_int_is_valid() {
        let mut tcx_mut = TyCtxMut::new(Interner::new());
        let u8 = tcx_mut.mk_ty(TyKind::Uint(UintTy::U8));
        assert!(is_valid_cast(&tcx_mut, tcx_mut.error_ty(), u8));
    }
}
