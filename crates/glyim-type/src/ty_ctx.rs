use crate::adt_def::*;
use crate::auto_trait::*;
use crate::display::TypeLookup;
use crate::flags::*;
use crate::region::*;
use crate::substitution::*;
use crate::ty::*;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::AdtId;
use glyim_core::interner::{Interner, Name};
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};

pub struct TyCtx {
    pub(crate) types: Vec<TyKind>,
    pub(crate) type_flags: Vec<TypeFlags>,
    pub(crate) substitution_data: Vec<SmallVec<[GenericArg; 4]>>,
    pub(crate) regions: IndexVec<RegionVid, Region>,
    pub(crate) resolver: Interner,
    pub(crate) auto_trait_registry: AutoTraitRegistry,
    pub(crate) adt_reprs: HashMap<AdtId, AdtRepr>,
    pub(crate) interior_mutable_adt_ids: HashSet<AdtId>,
    pub adt_defs: HashMap<AdtId, AdtDef>,
}

impl TyCtx {
    pub fn ty_kind(&self, ty: Ty) -> &TyKind {
        &self.types[ty.index()]
    }

    pub fn ty_flags(&self, ty: Ty) -> TypeFlags {
        self.type_flags[ty.index()]
    }

    pub fn substitution_args(&self, sub: Substitution) -> &[GenericArg] {
        &self.substitution_data[sub.index() as usize]
    }

    pub fn region(&self, vid: RegionVid) -> &Region {
        &self.regions[vid]
    }

    pub fn resolver(&self) -> &Interner {
        &self.resolver
    }

    pub fn name_str(&self, name: Name) -> &str {
        self.resolver.resolve(name)
    }

    pub fn is_copy(&self, ty: Ty) -> bool {
        match self.ty_kind(ty) {
            TyKind::Bool | TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_) | TyKind::Char => {
                true
            }
            TyKind::Never | TyKind::Unit => true,
            TyKind::Ref(_, _, _) => false,
            TyKind::RawPtr(_, _) => false,
            TyKind::Slice(_) => false,
            TyKind::Array(inner, _) => self.is_copy(*inner),
            TyKind::Tuple(substs) => {
                for arg in self.substitution_args(*substs) {
                    if let GenericArg::Ty(t) = arg
                        && !self.is_copy(*t)
                    {
                        return false;
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub fn error_ty(&self) -> Ty {
        Ty::ERROR
    }

    pub fn never_ty(&self) -> Ty {
        Ty::NEVER
    }

    pub fn unit_ty(&self) -> Ty {
        Ty::UNIT
    }

    pub fn bool_ty(&self) -> Ty {
        Ty::BOOL
    }

    pub fn ty_is_error(&self, ty: Ty) -> bool {
        self.ty_flags(ty).contains(TypeFlags::HAS_ERROR)
    }

    pub fn ty_has_depth_overflow(&self, ty: Ty) -> bool {
        self.ty_flags(ty).contains(TypeFlags::HAS_DEPTH_OVERFLOW)
    }

    pub fn auto_trait_flags(&self, ty: Ty) -> AutoTraitFlags {
        compute_auto_traits(ty, self, &self.auto_trait_registry, &self.adt_reprs)
    }

    pub fn implements_auto_trait(&self, ty: Ty, auto_trait: AutoTrait) -> bool {
        self.auto_trait_flags(ty).contains(auto_trait.flag())
    }

    pub fn has_negative_impl(&self, adt_id: AdtId, auto_trait: AutoTrait) -> bool {
        self.auto_trait_registry
            .has_negative_impl(adt_id, auto_trait)
    }

    pub fn has_manual_impl(&self, adt_id: AdtId, auto_trait: AutoTrait) -> bool {
        self.auto_trait_registry.has_manual_impl(adt_id, auto_trait)
    }

    pub fn adt_repr(&self, adt_id: AdtId) -> Option<&AdtRepr> {
        self.adt_reprs.get(&adt_id)
    }

    pub fn field_ty(&self, adt_id: AdtId, field_idx: usize) -> Ty {
        if let Some(repr) = self.adt_reprs.get(&adt_id) {
            repr.field_tys
                .get(field_idx)
                .copied()
                .unwrap_or_else(|| self.error_ty())
        } else {
            self.error_ty()
        }
    }

    pub fn adt_def(&self, id: AdtId) -> Option<&AdtDef> {
        self.adt_defs.get(&id)
    }

    pub fn field_index(&self, adt_id: AdtId, field_name: Name) -> Option<usize> {
        if let Some(def) = self.adt_defs.get(&adt_id) {
            for (i, field) in def.fields.iter_enumerated() {
                if field.name == field_name {
                    return Some(i.index());
                }
            }
        }
        None
    }
}

impl TypeLookup for TyCtx {
    fn ty_kind(&self, ty: Ty) -> &TyKind {
        &self.types[ty.index()]
    }
    fn ty_flags(&self, ty: Ty) -> TypeFlags {
        self.type_flags[ty.index()]
    }
    fn substitution_args(&self, sub: Substitution) -> &[GenericArg] {
        &self.substitution_data[sub.index() as usize]
    }
    fn name_str(&self, name: Name) -> &str {
        self.resolver.resolve(name)
    }
    fn error_ty(&self) -> Ty {
        Ty::ERROR
    }
    fn is_interior_mutable_adt(&self, adt_id: AdtId) -> bool {
        self.interior_mutable_adt_ids.contains(&adt_id)
    }
}
