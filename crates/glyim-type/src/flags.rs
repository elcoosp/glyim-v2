use crate::display::TypeLookup;
use crate::region::*;
use crate::substitution::*;
use crate::ty::*;
use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TypeFlags: u32 {
        const HAS_TY_INFER       = 1 << 0;
        const HAS_TY_PARAM       = 1 << 1;
        const HAS_RE_INFER       = 1 << 2;
        const HAS_RE_PARAM       = 1 << 3;
        const HAS_CT_INFER       = 1 << 4;
        const HAS_CT_PARAM       = 1 << 5;
        const HAS_ERROR          = 1 << 7;
        const HAS_DEPTH_OVERFLOW = 1 << 8;
    }
}

pub fn compute_flags(kind: &TyKind, ctx: &dyn TypeLookup, depth: u32) -> TypeFlags {
    const MAX_DEPTH: u32 = 64;
    if depth > MAX_DEPTH {
        tracing::warn!(
            "compute_flags exceeded depth limit at depth {}; TyKind summary: {:?}",
            depth,
            kind
        );
        return TypeFlags::HAS_DEPTH_OVERFLOW;
    }
    let mut flags = TypeFlags::empty();
    match kind {
        TyKind::Infer(_) => flags |= TypeFlags::HAS_TY_INFER,
        TyKind::Param(_) => flags |= TypeFlags::HAS_TY_PARAM,
        TyKind::Error => flags |= TypeFlags::HAS_ERROR,
        TyKind::Ref(region, ty, _) => {
            flags |= ctx.ty_flags(*ty);
            if let Region::Var(_) = region {
                flags |= TypeFlags::HAS_RE_INFER;
            }
            if let Region::EarlyBound(_) = region {
                flags |= TypeFlags::HAS_RE_PARAM;
            }
        }
        TyKind::RawPtr(ty, _) => flags |= ctx.ty_flags(*ty),
        TyKind::Slice(ty) => flags |= ctx.ty_flags(*ty),
        TyKind::Array(ty, _) => flags |= ctx.ty_flags(*ty),
        TyKind::Adt(_, substs)
        | TyKind::FnDef(_, substs)
        | TyKind::Closure(_, substs)
        | TyKind::Tuple(substs)
        | TyKind::Opaque(_, substs) => {
            for arg in ctx.substitution_args(*substs) {
                if let GenericArg::Ty(t) = arg {
                    flags |= ctx.ty_flags(*t);
                }
            }
        }
        TyKind::Projection(proj) => {
            for arg in ctx.substitution_args(proj.trait_ref.substs) {
                if let GenericArg::Ty(t) = arg {
                    flags |= ctx.ty_flags(*t);
                }
            }
        }
        TyKind::FnPtr(sig) => {
            for arg in ctx.substitution_args(sig.inputs) {
                if let GenericArg::Ty(t) = arg {
                    flags |= ctx.ty_flags(*t);
                }
            }
            flags |= ctx.ty_flags(sig.output);
        }
        _ => {}
    }
    flags
}
