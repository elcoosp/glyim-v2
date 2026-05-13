use std::fmt;
use crate::ty::*;
use crate::region::*;
use crate::substitution::*;
use crate::const_val::*;
use crate::fn_sig::*;
use crate::flags::TypeFlags;
use glyim_core::interner::Name;

pub trait TypeLookup {
    fn ty_kind(&self, ty: Ty) -> &TyKind;
    fn ty_flags(&self, ty: Ty) -> TypeFlags;
    fn substitution_args(&self, sub: Substitution) -> &[GenericArg];
    fn name_str(&self, name: Name) -> &str;
    fn error_ty(&self) -> Ty;
}

const MAX_DISPLAY_DEPTH: u32 = 10;

pub struct PrintTy<'a, L: TypeLookup> {
    ty: Ty,
    lookup: &'a L,
    depth: u32,
}

impl<'a, L: TypeLookup> PrintTy<'a, L> {
    pub fn new(ty: Ty, lookup: &'a L) -> Self {
        Self { ty, lookup, depth: 0 }
    }
    fn nested(&self, ty: Ty) -> Self {
        Self { ty, lookup: self.lookup, depth: self.depth + 1 }
    }
}

impl<L: TypeLookup> fmt::Display for PrintTy<'_, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.depth > MAX_DISPLAY_DEPTH {
            return write!(f, "…");
        }
        match self.lookup.ty_kind(self.ty) {
            TyKind::Bool => write!(f, "bool"),
            TyKind::Never => write!(f, "!"),
            TyKind::Unit => write!(f, "()"),
            TyKind::Int(i) => write!(f, "{}", i.name()),
            TyKind::Uint(u) => write!(f, "{}", u.name()),
            TyKind::Float(fl) => write!(f, "{}", fl.name()),
            TyKind::Error => write!(f, "<error>"),
            TyKind::Infer(InferVar::Ty(v)) => write!(f, "?ty{}", v.to_raw()),
            TyKind::Infer(InferVar::Int(v)) => write!(f, "?int{}", v.to_raw()),
            TyKind::Infer(InferVar::Float(v)) => write!(f, "?float{}", v.to_raw()),
            TyKind::Ref(_, ty, Mutability::Mut) => write!(f, "&mut {}", self.nested(*ty)),
            TyKind::Ref(_, ty, Mutability::Not) => write!(f, "&{}", self.nested(*ty)),
            _ => write!(f, "{:?}", self.lookup.ty_kind(self.ty)),
        }
    }
}

pub struct DebugTy<'a, L: TypeLookup>(pub PrintTy<'a, L>);

impl<L: TypeLookup> fmt::Debug for DebugTy<'_, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
