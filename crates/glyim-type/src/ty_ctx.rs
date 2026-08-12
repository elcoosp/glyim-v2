use crate::adt_def::*;
use crate::auto_trait::*;
use crate::display::TypeLookup;
use crate::flags::*;
use crate::fn_sig::FnSig;
use crate::region::*;
use crate::substitution::*;
use crate::ty::*;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::{AdtId, ClosureId, FnDefId, LocalDefId};
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
    pub(crate) trait_defs: HashMap<glyim_core::def_id::TraitDefId, crate::TraitDef>,
    pub(crate) variant_types: HashMap<AdtId, Vec<Ty>>,
    pub(crate) fn_sigs: HashMap<FnDefId, FnSig>,
    pub(crate) closure_sigs: HashMap<ClosureId, FnSig>,
    pub(crate) body_tys: HashMap<LocalDefId, Ty>,
}

impl TyCtx {
    pub fn ty_kind(&self, ty: Ty) -> &TyKind {
        &self.types[ty.index()]
    }

    pub fn ty_flags(&self, ty: Ty) -> TypeFlags {
        self.type_flags[ty.index()]
    }

    pub fn substitution_args(&self, sub: Substitution) -> &[GenericArg] {
        if sub.len() == 0 {
            return &[];
        }
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
        if let Some(def) = self.adt_defs.get(&adt_id) {
            return def
                .fields
                .as_slice()
                .get(field_idx)
                .map(|f| f.ty)
                .unwrap_or_else(|| self.error_ty());
        }
        if let Some(repr) = self.adt_reprs.get(&adt_id) {
            return repr
                .field_tys
                .get(field_idx)
                .copied()
                .unwrap_or_else(|| self.error_ty());
        }
        self.error_ty()
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

    pub fn fn_sig(&self, def_id: FnDefId) -> Option<&FnSig> {
        self.fn_sigs.get(&def_id)
    }

    pub fn closure_sig(&self, closure_id: ClosureId) -> Option<&FnSig> {
        self.closure_sigs.get(&closure_id)
    }

    pub fn body_ty(&self, def_id: LocalDefId) -> Option<Ty> {
        self.body_tys.get(&def_id).copied()
    }

    pub fn variant_type(&self, adt_id: AdtId, variant_idx: u32) -> Ty {
        self.variant_types
            .get(&adt_id)
            .and_then(|vts| vts.get(variant_idx as usize).copied())
            .unwrap_or(Ty::ERROR)
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
    fn adt_def(&self, adt_id: AdtId) -> Option<&AdtDef> {
        self.adt_defs.get(&adt_id)
    }
    fn field_ty(&self, adt_id: AdtId, field_idx: usize) -> Ty {
        if let Some(def) = self.adt_defs.get(&adt_id) {
            return def
                .fields
                .as_slice()
                .get(field_idx)
                .map(|f| f.ty)
                .unwrap_or_else(|| self.error_ty());
        }
        if let Some(repr) = self.adt_reprs.get(&adt_id) {
            return repr
                .field_tys
                .get(field_idx)
                .copied()
                .unwrap_or_else(|| self.error_ty());
        }
        self.error_ty()
    }
}
impl TyCtx {
    /// Get the trait definition by ID.
    pub fn trait_def(&self, id: glyim_core::def_id::TraitDefId) -> Option<&crate::TraitDef> {
        self.trait_defs.get(&id)
    }
}
