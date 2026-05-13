use glyim_core::arena::{IdxLike, IndexVec};
use glyim_core::interner::{Interner, Name};
use indexmap::IndexSet;
use smallvec::SmallVec;
use std::marker::PhantomData;
use crate::ty::*;
use crate::region::*;
use crate::substitution::*;
use crate::flags::*;
use crate::const_val::*;
use crate::fn_sig::*;
use crate::predicate::*;
use crate::binder::*;
use crate::display::TypeLookup;

pub struct TyCtxMut {
    types: Vec<TyKind>,
    type_flags: Vec<TypeFlags>,
    substitution_data: IndexSet<SmallVec<[GenericArg; 4]>>,
    regions: IndexVec<RegionVid, Region>,
    resolver: Interner,
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
            _not_send_sync: PhantomData,
        };
        // Pre-intern sentinels
        assert_eq!(ctx.alloc_ty_internal(TyKind::Error).to_raw(), Ty::ERROR.to_raw());
        assert_eq!(ctx.alloc_ty_internal(TyKind::Never).to_raw(), Ty::NEVER.to_raw());
        assert_eq!(ctx.alloc_ty_internal(TyKind::Unit).to_raw(), Ty::UNIT.to_raw());
        assert_eq!(ctx.alloc_ty_internal(TyKind::Bool).to_raw(), Ty::BOOL.to_raw());
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

    pub fn ty_kind(&self, ty: Ty) -> &TyKind { &self.types[ty.index()] }
    pub fn ty_kind_mut(&mut self, ty: Ty) -> &mut TyKind { &mut self.types[ty.index()] }
    pub fn ty_flags(&self, ty: Ty) -> TypeFlags { self.type_flags[ty.index()] }

    pub fn intern_substitution(&mut self, args: Vec<GenericArg>) -> Substitution {
        let small_args: SmallVec<[GenericArg; 4]> = args.into_iter().collect();
        let len = small_args.len() as u16;
        let (index, _) = self.substitution_data.insert_full(small_args);
        Substitution::from_raw(index as u32, len)
    }

    pub fn substitution_args(&self, sub: Substitution) -> &[GenericArg] {
        &self.substitution_data[sub.index() as usize]
    }

    pub fn mk_ty(&mut self, kind: TyKind) -> Ty { self.alloc_ty(kind) }
    pub fn mk_ref(&mut self, region: Region, ty: Ty, mutability: Mutability) -> Ty {
        self.mk_ty(TyKind::Ref(region, ty, mutability))
    }
    pub fn error_ty(&self) -> Ty { Ty::ERROR }
    pub fn never_ty(&self) -> Ty { Ty::NEVER }
    pub fn unit_ty(&self) -> Ty { Ty::UNIT }
    pub fn bool_ty(&self) -> Ty { Ty::BOOL }

    pub fn resolver(&self) -> &Interner { &self.resolver }
    pub fn name_str(&self, name: Name) -> &str { self.resolver.resolve(name) }

    pub fn freeze(self) -> TyCtx {
        TyCtx {
            types: self.types,
            type_flags: self.type_flags,
            substitution_data: self.substitution_data.into_iter().collect(),
            regions: self.regions,
            resolver: self.resolver,
        }
    }
}

impl TypeLookup for TyCtxMut {
    fn ty_kind(&self, ty: Ty) -> &TyKind { &self.types[ty.index()] }
    fn ty_flags(&self, ty: Ty) -> TypeFlags { self.type_flags[ty.index()] }
    fn substitution_args(&self, sub: Substitution) -> &[GenericArg] { &self.substitution_data[sub.index() as usize] }
    fn name_str(&self, name: Name) -> &str { self.resolver.resolve(name) }
    fn error_ty(&self) -> Ty { Ty::ERROR }
}

pub struct TyCtx {
    types: Vec<TyKind>,
    type_flags: Vec<TypeFlags>,
    substitution_data: Vec<SmallVec<[GenericArg; 4]>>,
    regions: IndexVec<RegionVid, Region>,
    resolver: Interner,
}

impl TyCtx {
    pub fn ty_kind(&self, ty: Ty) -> &TyKind { &self.types[ty.index()] }
    pub fn ty_flags(&self, ty: Ty) -> TypeFlags { self.type_flags[ty.index()] }
    pub fn substitution_args(&self, sub: Substitution) -> &[GenericArg] { &self.substitution_data[sub.index() as usize] }
    pub fn region(&self, vid: RegionVid) -> &Region { &self.regions[vid] }
    pub fn resolver(&self) -> &Interner { &self.resolver }
    pub fn name_str(&self, name: Name) -> &str { self.resolver.resolve(name) }
    pub fn error_ty(&self) -> Ty { Ty::ERROR }
    pub fn never_ty(&self) -> Ty { Ty::NEVER }
    pub fn unit_ty(&self) -> Ty { Ty::UNIT }
    pub fn bool_ty(&self) -> Ty { Ty::BOOL }
    pub fn ty_is_error(&self, ty: Ty) -> bool { self.ty_flags(ty).contains(TypeFlags::HAS_ERROR) }
    pub fn ty_has_depth_overflow(&self, ty: Ty) -> bool { self.ty_flags(ty).contains(TypeFlags::HAS_DEPTH_OVERFLOW) }
}

impl TypeLookup for TyCtx {
    fn ty_kind(&self, ty: Ty) -> &TyKind { &self.types[ty.index()] }
    fn ty_flags(&self, ty: Ty) -> TypeFlags { self.type_flags[ty.index()] }
    fn substitution_args(&self, sub: Substitution) -> &[GenericArg] { &self.substitution_data[sub.index() as usize] }
    fn name_str(&self, name: Name) -> &str { self.resolver.resolve(name) }
    fn error_ty(&self) -> Ty { Ty::ERROR }
}
