use glyim_core::arena::IndexVec;
use glyim_core::def_id::AdtId;
use glyim_core::interner::{Interner, Name};
use glyim_core::primitives::Mutability;

use crate::auto_trait::*;
use crate::display::TypeLookup;
use crate::flags::*;
use crate::fn_sig::FnSig;
use crate::region::*;
use crate::substitution::*;
use crate::ty::*;

use indexmap::IndexSet;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::marker::PhantomData;

pub struct TyCtxMut {
    types: Vec<TyKind>,
    type_flags: Vec<TypeFlags>,
    substitution_data: IndexSet<SmallVec<[GenericArg; 4]>>,
    regions: IndexVec<RegionVid, Region>,
    resolver: Interner,
    auto_trait_registry: AutoTraitRegistry,
    adt_reprs: HashMap<AdtId, AdtRepr>,
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
            _not_send_sync: PhantomData,
        };
        // Pre-intern sentinels — must be in this order:
        // Ty::ERROR=0, Ty::NEVER=1, Ty::UNIT=2, Ty::BOOL=3
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

    /// Allocate a new region variable with the given initial value.
    pub fn new_region_var(&mut self, initial: Region) -> RegionVid {
        self.regions.push(initial)
    }

    /// Retrieve the region associated with a region variable.
    pub fn region_var(&self, vid: RegionVid) -> &Region {
        &self.regions[vid]
    }

    /// Return the number of allocated region variables.
    pub fn region_var_count(&self) -> usize {
        self.regions.len()
    }

    /// Register the field types for an ADT, used for auto trait computation.
    pub fn register_adt_repr(&mut self, adt_id: AdtId, field_tys: Vec<Ty>) {
        self.adt_reprs.insert(adt_id, AdtRepr::new(field_tys));
    }

    /// Register a negative auto trait impl: `impl !Trait for Adt`.
    pub fn register_negative_impl(&mut self, adt_id: AdtId, auto_trait: AutoTrait) {
        self.auto_trait_registry
            .register_negative_impl(adt_id, auto_trait);
    }

    /// Register a manual auto trait impl: `impl Trait for Adt`.
    pub fn register_manual_impl(&mut self, adt_id: AdtId, auto_trait: AutoTrait) {
        self.auto_trait_registry
            .register_manual_impl(adt_id, auto_trait);
    }

    pub fn freeze(self) -> TyCtx {
        TyCtx {
            types: self.types,
            type_flags: self.type_flags,
            substitution_data: self.substitution_data.into_iter().collect(),
            regions: self.regions,
            resolver: self.resolver,
            auto_trait_registry: self.auto_trait_registry,
            adt_reprs: self.adt_reprs,
        }
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
}

pub struct TyCtx {
    types: Vec<TyKind>,
    type_flags: Vec<TypeFlags>,
    substitution_data: Vec<SmallVec<[GenericArg; 4]>>,
    regions: IndexVec<RegionVid, Region>,
    resolver: Interner,
    auto_trait_registry: AutoTraitRegistry,
    adt_reprs: HashMap<AdtId, AdtRepr>,
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

    /// Compute auto trait flags for a type using this frozen context.
    pub fn auto_trait_flags(&self, ty: Ty) -> AutoTraitFlags {
        compute_auto_traits(ty, self, &self.auto_trait_registry, &self.adt_reprs)
    }

    /// Check whether a type implements a specific auto trait.
    pub fn implements_auto_trait(&self, ty: Ty, auto_trait: AutoTrait) -> bool {
        self.auto_trait_flags(ty).contains(auto_trait.flag())
    }

    /// Check if a specific ADT has a negative impl for an auto trait.
    pub fn has_negative_impl(&self, adt_id: AdtId, auto_trait: AutoTrait) -> bool {
        self.auto_trait_registry
            .has_negative_impl(adt_id, auto_trait)
    }

    /// Check if a specific ADT has a manual impl for an auto trait.
    pub fn has_manual_impl(&self, adt_id: AdtId, auto_trait: AutoTrait) -> bool {
        self.auto_trait_registry.has_manual_impl(adt_id, auto_trait)
    }

    /// Access the ADT representation (field types).
    pub fn adt_repr(&self, adt_id: AdtId) -> Option<&AdtRepr> {
        self.adt_reprs.get(&adt_id)
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
}
