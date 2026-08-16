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
use glyim_core::primitives::{IntTy, Mutability, UintTy};
use indexmap::IndexSet;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::collections::HashSet;

pub struct TyCtxMut {
    types: Vec<TyKind>,
    type_flags: Vec<TypeFlags>,
    substitution_data: IndexSet<SmallVec<[GenericArg; 4]>>,
    regions: IndexVec<RegionVid, Region>,
    resolver: Interner,
    auto_trait_registry: AutoTraitRegistry,
    adt_reprs: HashMap<AdtId, AdtRepr>,
    interior_mutable_adt_ids: HashSet<AdtId>,
    interior_mutability_cache: HashMap<AdtId, bool>,
    adt_defs: HashMap<AdtId, AdtDef>,

    pub(crate) trait_defs: HashMap<glyim_core::def_id::TraitDefId, crate::TraitDef>,

    variant_types: HashMap<AdtId, Vec<Ty>>,
    fn_sigs: HashMap<FnDefId, FnSig>,
    closure_sigs: HashMap<ClosureId, FnSig>,
    body_tys: HashMap<LocalDefId, Ty>,
    /// Counter for compiler-synthesized ADT ids (closures, etc.). Seeded far
    /// above any user-defined ADT id and the 1000-1005 builtin range ids so it
    /// can never collide.
    synthetic_adt_counter: u32,
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
            interior_mutability_cache: HashMap::new(),
            adt_defs: HashMap::new(),
            trait_defs: HashMap::new(),
            variant_types: HashMap::new(),
            fn_sigs: HashMap::new(),
            closure_sigs: HashMap::new(),
            body_tys: HashMap::new(),
            synthetic_adt_counter: 2_000_000,
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
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Uint(UintTy::U8)).to_raw(),
            Ty::U8.to_raw(),
            "Ty::U8 sentinel mismatch"
        );
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Uint(UintTy::U16)).to_raw(),
            Ty::U16.to_raw(),
            "Ty::U16 sentinel mismatch"
        );
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Uint(UintTy::U32)).to_raw(),
            Ty::U32.to_raw(),
            "Ty::U32 sentinel mismatch"
        );
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Uint(UintTy::U64)).to_raw(),
            Ty::U64.to_raw(),
            "Ty::U64 sentinel mismatch"
        );
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Uint(UintTy::Usize)).to_raw(),
            Ty::USIZE.to_raw(),
            "Ty::USIZE sentinel mismatch"
        );
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Int(IntTy::I8)).to_raw(),
            Ty::I8.to_raw(),
            "Ty::I8 sentinel mismatch"
        );
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Int(IntTy::I16)).to_raw(),
            Ty::I16.to_raw(),
            "Ty::I16 sentinel mismatch"
        );
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Int(IntTy::I32)).to_raw(),
            Ty::I32.to_raw(),
            "Ty::I32 sentinel mismatch"
        );
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Int(IntTy::I64)).to_raw(),
            Ty::I64.to_raw(),
            "Ty::I64 sentinel mismatch"
        );
        assert_eq!(
            ctx.alloc_ty_internal(TyKind::Int(IntTy::Isize)).to_raw(),
            Ty::ISIZE.to_raw(),
            "Ty::ISIZE sentinel mismatch"
        );
        ctx.register_builtin_ranges();
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
        if sub.is_empty() {
            return &[];
        }
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
        // Compute variant types from variants
        let variant_tys: Vec<Ty> = def
            .variants
            .iter()
            .map(|variant| {
                let field_tys: Vec<Ty> = variant.fields.iter().map(|f| f.ty).collect();
                if field_tys.is_empty() {
                    self.unit_ty()
                } else if field_tys.len() == 1 {
                    field_tys[0]
                } else {
                    let args: Vec<GenericArg> = field_tys.into_iter().map(GenericArg::Ty).collect();
                    let subst = self.intern_substitution(args);
                    self.mk_ty(TyKind::Tuple(subst))
                }
            })
            .collect();
        self.variant_types.insert(id, variant_tys);
        self.adt_defs.insert(id, def.clone());
        if self.compute_adt_interior_mutability(id) {
            self.mark_adt_interior_mutable(id);
        }
    }

    /// Allocate a fresh compiler-synthesized `AdtId` that cannot collide with
    /// user-defined ADTs or the builtin fixed-id ADTs (ranges, etc.).
    pub fn next_synthetic_adt_id(&mut self) -> AdtId {
        let id = self.synthetic_adt_counter;
        self.synthetic_adt_counter += 1;
        AdtId::from_raw(id)
    }

    /// Register a nominal ADT representing a closure's captured environment.
    ///
    /// Each distinct closure expression (with a distinct capture set) gets its
    /// own synthetic ADT with one field per captured variable. This gives a
    /// closure a real, concrete `TyKind::Adt` so downstream passes (MIR
    /// lowering, layout, codegen) have a genuine type to work with instead of
    /// an unconstrained inference variable.
    pub fn register_closure(&mut self, capture_tys: Vec<Ty>) -> AdtId {
        let id = self.next_synthetic_adt_id();
        let mut field_defs = IndexVec::new();
        for (i, ty) in capture_tys.iter().enumerate() {
            field_defs.push(FieldDef {
                name: self.resolver.intern(&format!("capture_{i}")),
                ty: *ty,
            });
        }
        let def = AdtDef {
            kind: AdtKind::Struct,
            fields: field_defs.clone(),
            variants: vec![VariantDef {
                name: self.resolver.intern(""),
                fields: field_defs,
            }],
        };
        self.register_adt(id, def);
        id
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

    pub fn register_fn_sig(&mut self, def_id: FnDefId, sig: FnSig) {
        self.fn_sigs.insert(def_id, sig);
    }

    /// Register a trait definition (its name and method set) so that vtable
    /// generation and other downstream passes can resolve the trait's methods.
    pub fn register_trait_def(&mut self, id: glyim_core::def_id::TraitDefId, def: crate::TraitDef) {
        self.trait_defs.insert(id, def);
    }

    pub fn fn_sig(&self, def_id: FnDefId) -> Option<&FnSig> {
        self.fn_sigs.get(&def_id)
    }

    pub fn register_closure_sig(&mut self, closure_id: ClosureId, sig: FnSig) {
        self.closure_sigs.insert(closure_id, sig);
    }

    pub fn closure_sig(&self, closure_id: ClosureId) -> Option<&FnSig> {
        self.closure_sigs.get(&closure_id)
    }

    pub fn register_body_ty(&mut self, def_id: LocalDefId, ty: Ty) {
        self.body_tys.insert(def_id, ty);
    }

    pub fn body_ty(&self, def_id: LocalDefId) -> Option<Ty> {
        self.body_tys.get(&def_id).copied()
    }

    pub fn freeze(&self) -> super::ty_ctx::TyCtx {
        super::ty_ctx::TyCtx {
            types: self.types.clone(),
            type_flags: self.type_flags.clone(),
            substitution_data: self.substitution_data.iter().cloned().collect(),
            regions: self.regions.clone(),
            resolver: self.resolver.clone(),
            auto_trait_registry: self.auto_trait_registry.clone(),
            adt_reprs: self.adt_reprs.clone(),
            interior_mutable_adt_ids: self.interior_mutable_adt_ids.clone(),
            adt_defs: self.adt_defs.clone(),
            trait_defs: self.trait_defs.clone(),
            variant_types: self.variant_types.clone(),
            fn_sigs: self.fn_sigs.clone(),
            closure_sigs: self.closure_sigs.clone(),
            body_tys: self.body_tys.clone(),
        }
    }

    pub fn freeze_owned(self) -> super::ty_ctx::TyCtx {
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
            trait_defs: self.trait_defs.clone(),
            variant_types: self.variant_types,
            fn_sigs: self.fn_sigs,
            closure_sigs: self.closure_sigs,
            body_tys: self.body_tys,
        }
    }

    pub fn mark_adt_interior_mutable(&mut self, adt_id: AdtId) {
        self.interior_mutable_adt_ids.insert(adt_id);
        self.interior_mutability_cache.insert(adt_id, true);
    }

    fn compute_adt_interior_mutability(&mut self, adt_id: AdtId) -> bool {
        if let Some(&cached) = self.interior_mutability_cache.get(&adt_id) {
            return cached;
        }
        if self.interior_mutable_adt_ids.contains(&adt_id) {
            self.interior_mutability_cache.insert(adt_id, true);
            return true;
        }
        let def = match self.adt_defs.get(&adt_id) {
            Some(def) => def.clone(),
            None => {
                self.interior_mutability_cache.insert(adt_id, false);
                return false;
            }
        };
        let result = self.compute_adt_interior_mutability_with_def(adt_id, &def);
        self.interior_mutability_cache.insert(adt_id, result);
        result
    }

    fn compute_adt_interior_mutability_with_def(&mut self, adt_id: AdtId, def: &AdtDef) -> bool {
        let mut visiting = HashSet::new();
        self.compute_adt_interior_mutability_rec(adt_id, def, &mut visiting)
    }

    fn compute_adt_interior_mutability_rec(
        &mut self,
        adt_id: AdtId,
        def: &AdtDef,
        visiting: &mut HashSet<AdtId>,
    ) -> bool {
        if visiting.contains(&adt_id) {
            return false;
        }
        visiting.insert(adt_id);
        for field in def.fields.iter() {
            let field_ty = field.ty;
            let flags = self.ty_flags(field_ty);
            if flags.contains(TypeFlags::HAS_INTERIOR_MUTABILITY) {
                visiting.remove(&adt_id);
                return true;
            }
            if let TyKind::Adt(child_adt_id, _) = self.ty_kind(field_ty)
                && self.compute_adt_interior_mutability(*child_adt_id)
            {
                visiting.remove(&adt_id);
                return true;
            }
        }
        visiting.remove(&adt_id);
        false
    }

    /// Register built-in range types as ADTs.
    pub fn register_builtin_ranges(&mut self) {
        use crate::adt_def::{AdtDef, AdtKind, FieldDef, VariantDef};
        use glyim_core::arena::IndexVec;
        use glyim_core::def_id::AdtId;

        // Helper to create a struct AdtDef with given field types.
        // This is a plain function, not a closure, so it doesn't capture self.
        // But we can't define a nested function that uses self.resolver directly.
        // We'll just create each def manually.

        // Create a type variable for T.
        let t_var = self.mk_ty(TyKind::Param(ParamTy {
            index: 0,
            name: self.resolver.intern("T"),
        }));

        // Helper to make field defs for a list of types.
        // We'll call self.resolver.intern directly inside the loop.
        // This is safe because we don't hold any borrows across calls.
        let make_field_defs = |fields: Vec<Ty>, this: &mut Self| {
            let mut field_defs = IndexVec::new();
            for (i, ty) in fields.iter().enumerate() {
                field_defs.push(FieldDef {
                    name: this.resolver.intern(&format!("field_{}", i)),
                    ty: *ty,
                });
            }
            field_defs
        };

        // Helper to make a struct AdtDef.
        let make_struct_def = |fields: Vec<Ty>, this: &mut Self| {
            let field_defs = make_field_defs(fields, this);
            let field_defs_clone = field_defs.clone();
            AdtDef {
                kind: AdtKind::Struct,
                fields: field_defs,
                variants: vec![VariantDef {
                    name: this.resolver.intern(""),
                    fields: field_defs_clone,
                }],
            }
        };

        // Register Range<T> (start, end) - ID 1000
        let def = make_struct_def(vec![t_var, t_var], self);
        self.register_adt(AdtId::from_raw(1000), def);

        // Register RangeInclusive<T> (start, end) - ID 1001
        let def = make_struct_def(vec![t_var, t_var], self);
        self.register_adt(AdtId::from_raw(1001), def);

        // Register RangeFrom<T> (start) - ID 1002
        let def = make_struct_def(vec![t_var], self);
        self.register_adt(AdtId::from_raw(1002), def);

        // Register RangeTo<T> (end) - ID 1003
        let def = make_struct_def(vec![t_var], self);
        self.register_adt(AdtId::from_raw(1003), def);

        // Register RangeToInclusive<T> (end) - ID 1004
        let def = make_struct_def(vec![t_var], self);
        self.register_adt(AdtId::from_raw(1004), def);

        // Register UnsafeCell<T> - ID 1005
        // This is a special ADT for interior mutability.
        let unsafe_cell_t_var = self.mk_ty(TyKind::Param(ParamTy {
            index: 0,
            name: self.resolver.intern("T"),
        }));
        let unsafe_cell_def = {
            let mut field_defs = IndexVec::new();
            // UnsafeCell has a single field: value of type T.
            field_defs.push(FieldDef {
                name: self.resolver.intern("value"),
                ty: unsafe_cell_t_var,
            });
            let field_defs_clone = field_defs.clone();
            AdtDef {
                kind: AdtKind::Struct,
                fields: field_defs,
                variants: vec![VariantDef {
                    name: self.resolver.intern(""),
                    fields: field_defs_clone,
                }],
            }
        };
        self.register_adt(AdtId::from_raw(1005), unsafe_cell_def);
        // Mark it as interior mutable.
        self.mark_adt_interior_mutable(AdtId::from_raw(1005));

        // Register Option<T> - ID 1010
        // A two-variant enum: `None` (no fields) and `Some(T)`.
        let none_fields = IndexVec::new();
        let mut some_fields = IndexVec::new();
        some_fields.push(FieldDef {
            name: self.resolver.intern("0"),
            ty: t_var,
        });
        let option_def = AdtDef {
            kind: AdtKind::Enum,
            fields: IndexVec::new(),
            variants: vec![
                VariantDef {
                    name: self.resolver.intern("None"),
                    fields: none_fields,
                },
                VariantDef {
                    name: self.resolver.intern("Some"),
                    fields: some_fields.clone(),
                },
            ],
        };
        self.register_adt(AdtId::from_raw(1010), option_def);
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

#[cfg(test)]
mod interior_mutability_tests {
    use super::*;
    use glyim_core::interner::Interner;

    #[test]
    fn test_unsafe_cell_is_interior_mutable() {
        let ctx_mut = TyCtxMut::new(Interner::new());
        // register_builtin_ranges is called in new, so UnsafeCell (1005) is registered.
        let ctx = ctx_mut.freeze();
        let unsafe_cell_adt = AdtId::from_raw(1005);
        assert!(ctx.is_interior_mutable_adt(unsafe_cell_adt));
    }

    #[test]
    fn test_cell_is_interior_mutable() {
        // Cell<T> contains UnsafeCell<T> internally.
        let mut ctx_mut = TyCtxMut::new(Interner::new());
        let t_var = ctx_mut.mk_ty(TyKind::Param(ParamTy {
            index: 0,
            name: ctx_mut.resolver.intern("T"),
        }));
        // Build substitution for UnsafeCell<T>
        let subst = ctx_mut.intern_substitution(vec![GenericArg::Ty(t_var)]);
        let unsafe_cell_ty = ctx_mut.mk_ty(TyKind::Adt(AdtId::from_raw(1005), subst));
        let cell_def = {
            let mut field_defs = IndexVec::new();
            field_defs.push(FieldDef {
                name: ctx_mut.resolver.intern("value"),
                ty: unsafe_cell_ty,
            });
            let field_defs_clone = field_defs.clone();
            AdtDef {
                kind: AdtKind::Struct,
                fields: field_defs,
                variants: vec![VariantDef {
                    name: ctx_mut.resolver.intern(""),
                    fields: field_defs_clone,
                }],
            }
        };
        ctx_mut.register_adt(AdtId::from_raw(1006), cell_def);
        let ctx = ctx_mut.freeze();
        let cell_adt = AdtId::from_raw(1006);
        assert!(ctx.is_interior_mutable_adt(cell_adt));
    }

    #[test]
    fn test_option_builtin_registered() {
        // `Option<T>` is registered as a two-variant enum at id 1010 by
        // register_builtin_ranges (called in `new`).
        let mut ctx_mut = TyCtxMut::new(Interner::new());
        // Build Option<T> type to confirm the ADT exists and is an enum.
        let t_var = ctx_mut.mk_ty(TyKind::Param(ParamTy {
            index: 0,
            name: ctx_mut.resolver.intern("T"),
        }));
        let opt_subst = ctx_mut.intern_substitution(vec![GenericArg::Ty(t_var)]);
        let option_ty = ctx_mut.mk_ty(TyKind::Adt(AdtId::from_raw(1010), opt_subst));
        let ctx = ctx_mut.freeze();
        match ctx.ty_kind(option_ty) {
            TyKind::Adt(adt_id, _) => {
                assert_eq!(*adt_id, AdtId::from_raw(1010));
                let def = ctx.adt_def(*adt_id).expect("Option ADT should be registered");
                assert_eq!(def.variants.len(), 2, "Option has None + Some variants");
                assert_eq!(ctx.name_str(def.variants[0].name), "None");
                assert_eq!(ctx.name_str(def.variants[1].name), "Some");
            }
            other => panic!("expected Option ADT, got {other:?}"),
        }
    }

    #[test]
    fn test_plain_struct_not_interior_mutable() {
        let mut ctx_mut = TyCtxMut::new(Interner::new());
        let i32_ty = ctx_mut.mk_ty(TyKind::Int(glyim_core::primitives::IntTy::I32));
        let plain_def = {
            let mut field_defs = IndexVec::new();
            field_defs.push(FieldDef {
                name: ctx_mut.resolver.intern("x"),
                ty: i32_ty,
            });
            let field_defs_clone = field_defs.clone();
            AdtDef {
                kind: AdtKind::Struct,
                fields: field_defs,
                variants: vec![VariantDef {
                    name: ctx_mut.resolver.intern(""),
                    fields: field_defs_clone,
                }],
            }
        };
        ctx_mut.register_adt(AdtId::from_raw(1007), plain_def);
        let ctx = ctx_mut.freeze();
        let plain_adt = AdtId::from_raw(1007);
        assert!(!ctx.is_interior_mutable_adt(plain_adt));
    }

    #[test]
    fn test_ref_cell_is_interior_mutable() {
        // RefCell<T> contains UnsafeCell<T> internally.
        let mut ctx_mut = TyCtxMut::new(Interner::new());
        let t_var = ctx_mut.mk_ty(TyKind::Param(ParamTy {
            index: 0,
            name: ctx_mut.resolver.intern("T"),
        }));
        let subst = ctx_mut.intern_substitution(vec![GenericArg::Ty(t_var)]);
        let unsafe_cell_ty = ctx_mut.mk_ty(TyKind::Adt(AdtId::from_raw(1005), subst));
        let ref_cell_def = {
            let mut field_defs = IndexVec::new();
            field_defs.push(FieldDef {
                name: ctx_mut.resolver.intern("value"),
                ty: unsafe_cell_ty,
            });
            let field_defs_clone = field_defs.clone();
            AdtDef {
                kind: AdtKind::Struct,
                fields: field_defs,
                variants: vec![VariantDef {
                    name: ctx_mut.resolver.intern(""),
                    fields: field_defs_clone,
                }],
            }
        };
        ctx_mut.register_adt(AdtId::from_raw(1008), ref_cell_def);
        let ctx = ctx_mut.freeze();
        let ref_cell_adt = AdtId::from_raw(1008);
        assert!(ctx.is_interior_mutable_adt(ref_cell_adt));
    }
}
