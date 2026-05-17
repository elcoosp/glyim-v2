use crate::adt_def::*;
use crate::auto_trait::*;
use crate::display::TypeLookup;
use crate::flags::*;
use crate::fn_sig::FnSig;
use crate::region::*;
use crate::substitution::*;
use crate::ty::*;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::AdtId;
use glyim_core::interner::{Interner, Name};
use glyim_core::primitives::Mutability;
use indexmap::IndexSet;
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

pub struct TyCtxMut {
    types: Vec<TyKind>,
    type_flags: Vec<TypeFlags>,
    substitution_data: IndexSet<SmallVec<[GenericArg; 4]>>,
    regions: IndexVec<RegionVid, Region>,
    resolver: Interner,
    auto_trait_registry: AutoTraitRegistry,
    adt_reprs: HashMap<AdtId, AdtRepr>,
    interior_mutable_adt_ids: HashSet<AdtId>,
    adt_defs: HashMap<AdtId, AdtDef>,
    _not_send_sync: PhantomData<*const ()>,
}

impl TyCtxMut {
    pub fn new(resolver: Interner) -> Self {
        let mut ctx = Self {
            types: Vec::new(),
            type_flags: Vec::new(),
            substitution_data: IndexSet::new(),
            regions: IndexVec::new(),
            resolver,
            auto_trait_registry: AutoTraitRegistry::new(),
            adt_reprs: HashMap::new(),
            interior_mutable_adt_ids: HashSet::new(),
            adt_defs: HashMap::new(),
            _not_send_sync: PhantomData,
        };
        // sentinels
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Error).to_raw(),
            Ty::ERROR.to_raw(),
            "Ty::ERROR sentinel mismatch"
        );
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Never).to_raw(),
            Ty::NEVER.to_raw(),
            "Ty::NEVER sentinel mismatch"
        );
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Unit).to_raw(),
            Ty::UNIT.to_raw(),
            "Ty::UNIT sentinel mismatch"
        );
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Bool).to_raw(),
            Ty::BOOL.to_raw(),
            "Ty::BOOL sentinel mismatch"
        );
        ctx
    }

    fn alloc_ty_internal(&mut self, kind: TyKind) -> Ty {
        let flags = compute_flags(&kind, self, 0);
        let idx = self.types.len() as u32;
        self.types.push(kind);
        self.type_flags.push(flags);
        Ty::from_raw(idx)
    }

    pub fn alloc_ty(&mut self, kind: TyKind) -> Ty {
        self.alloc_ty_internal(kind)
    }

    pub fn ty_kind(&self, ty: Ty) -> &TyKind {
        &self.types[ty.index()]
    }

    pub fn ty_kind_mut(&mut self, ty: Ty) -> &mut TyKind {
        &mut self.types[ty.index()]
    }

    pub fn ty_flags(&self, ty: Ty) -> TypeFlags {
        self.type_flags[ty.index()]
    }

    pub fn intern_substitution(&mut self, args: Vec<GenericArg>) -> Substitution {
        let small_args: SmallVec<[GenericArg; 4]> = args.into_iter().collect();
        let len = small_args.len() as u16;
        let (index, _) = self.substitution_data.insert_full(small_args);
        Substitution::from_raw(index as u32, len)
    }

    pub fn substitution_args(&self, sub: Substitution) -> &[GenericArg] {
        &self.substitution_data[sub.index() as usize]
    }

    pub fn mk_ty(&mut self, kind: TyKind) -> Ty {
        self.alloc_ty(kind)
    }

    pub fn mk_ref(&mut self, region: Region, ty: Ty, mutability: Mutability) -> Ty {
        self.mk_ty(TyKind::Ref(region, ty, mutability))
    }

    pub fn mk_adt(&mut self, adt_id: AdtId, substs: Substitution) -> Ty {
        self.mk_ty(TyKind::Adt(adt_id, substs))
    }

    pub fn mk_tuple(&mut self, substs: Substitution) -> Ty {
        self.mk_ty(TyKind::Tuple(substs))
    }

    pub fn mk_fn_ptr(&mut self, sig: FnSig) -> Ty {
        self.mk_ty(TyKind::FnPtr(sig))
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

    pub fn new_region_var(&mut self, initial: Region) -> RegionVid {
        self.regions.push(initial)
    }

    pub fn region_var(&self, vid: RegionVid) -> &Region {
        &self.regions[vid]
    }

    pub fn region_var_count(&self) -> usize {
        self.regions.len()
    }

    pub fn register_adt_repr(&mut self, adt_id: AdtId, field_tys: Vec<Ty>) {
        self.adt_reprs.insert(adt_id, AdtRepr::new(field_tys));
    }

    pub fn register_negative_impl(&mut self, adt_id: AdtId, auto_trait: AutoTrait) {
        self.auto_trait_registry
            .register_negative_impl(adt_id, auto_trait);
    }

    pub fn register_manual_impl(&mut self, adt_id: AdtId, auto_trait: AutoTrait) {
        self.auto_trait_registry
            .register_manual_impl(adt_id, auto_trait);
    }

    pub fn register_adt(&mut self, id: AdtId, def: AdtDef) {
        self.adt_defs.insert(id, def);
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

    pub fn freeze(self) -> super::ty_ctx::TyCtx {
        super::ty_ctx::TyCtx {
            types: self.types,
            type_flags: self.type_flags,
            substitution_data: self.substitution_data.into_iter().collect(),
            regions: self.regions,
            resolver: self.resolver,
            auto_trait_registry: self.auto_trait_registry,
            adt_reprs: self.adt_reprs,
            interior_mutable_adt_ids: self.interior_mutable_adt_ids,
            adt_defs: self.adt_defs,
        }
    }

    pub fn mark_adt_interior_mutable(&mut self, adt_id: AdtId) {
        self.interior_mutable_adt_ids.insert(adt_id);
    }
}

impl TypeLookup for TyCtxMut {
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
