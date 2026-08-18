use crate::adt_def::*;
use crate::auto_trait::*;
use crate::display::TypeLookup;
use crate::flags::*;
use crate::fn_sig::FnSig;
use crate::lang_items::LangItems;
use crate::region::*;
use crate::substitution::*;
use crate::ty::*;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::{AdtId, ClosureId, ConstDefId, FnDefId, LocalDefId, OpaqueTyId};
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
    /// Concrete hidden types for opaque types (`impl Trait` / `type X = impl
    /// Trait`). Populated at the opaque type's defining use (by typeck) so that
    /// auto-trait computation (§7.2) can recurse into the underlying type
    /// instead of assuming zero auto traits.
    pub(crate) opaque_hidden: HashMap<OpaqueTyId, Ty>,
    pub(crate) interior_mutable_adt_ids: HashSet<AdtId>,
    pub adt_defs: HashMap<AdtId, AdtDef>,
    pub(crate) trait_defs: HashMap<glyim_core::def_id::TraitDefId, crate::TraitDef>,
    pub(crate) variant_types: HashMap<AdtId, Vec<Ty>>,
    pub(crate) fn_sigs: HashMap<FnDefId, FnSig>,
    pub(crate) const_tys: HashMap<ConstDefId, Ty>,
    pub(crate) closure_sigs: HashMap<ClosureId, FnSig>,
    pub(crate) body_tys: HashMap<LocalDefId, Ty>,
    pub(crate) lang_items: LangItems,
    /// `AdtId`s that have an explicit `Drop` impl (or are owning builtins such as
    /// `String`/`Vec`/`Box`). Consulted by `needs_drop` so that a type whose
    /// *fields* are all primitives (e.g. `String` backed by a raw pointer) is
    /// still correctly reported as needing drop glue. Populated via
    /// `TyCtxMut::mark_has_drop`.
    pub(crate) drop_impls: HashSet<AdtId>,
}

impl TyCtx {
    pub fn ty_kind(&self, ty: Ty) -> &TyKind {
        &self.types[ty.index()]
    }

    /// Access the language-item registry (builtin `Option`/`Range`/`Drop`/…).
    pub fn lang_items(&self) -> &LangItems {
        &self.lang_items
    }

    pub fn ty_flags(&self, ty: Ty) -> TypeFlags {
        self.type_flags[ty.index()]
    }

    pub fn substitution_args(&self, sub: Substitution) -> &[GenericArg] {
        if sub.is_empty() {
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

    /// Whether `ty` is `Sized` (has a known, non-zero/complete layout).
    ///
    /// Everything is sized except the genuinely unsized kinds: `str`, slice `[T]`,
    /// `dyn Trait` (trait objects), and ADTs whose last field is itself unsized
    /// (recursively). This is the structural `Sized` check used by the trait
    /// solver for `T: Sized` bounds (de-stubbing plan §8.1).
    pub fn is_sized(&self, ty: Ty) -> bool {
        match self.ty_kind(ty) {
            TyKind::Slice(_) | TyKind::Dynamic(..) | TyKind::Opaque(..) => false,
            TyKind::Array(inner, _) => self.is_sized(*inner),
            TyKind::Tuple(substs) => {
                self.substitution_args(*substs)
                    .iter()
                    .all(|arg| match arg {
                        GenericArg::Ty(t) => self.is_sized(*t),
                        _ => true,
                    })
            }
            TyKind::Adt(adt_id, _) => {
                if let Some(adt_def) = self.adt_def(*adt_id) {
                    // An ADT is sized unless it has a field that is unsized
                    // (the lang-level rule for the by-value-last-field DST).
                    adt_def.variants.iter().all(|v| {
                        v.fields
                            .iter()
                            .all(|f| self.is_sized(f.ty))
                    })
                } else {
                    // Unknown/unregistered ADT: assume sized (matches the
                    // conservative default used elsewhere for unregistered types).
                    true
                }
            }
            _ => true,
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

    /// Mechanically dereference a type for auto-deref in method resolution
    /// (de-stubbing plan §9.1). Handles the structural cases that need no trait
    /// database: shared/mutable references and raw pointers. ADT `Deref` impls
    /// (e.g. `Box<T>`, `Rc<T>`) require the `Deref` trait impl to be registered,
    /// which is gated on the HIR → `TraitContext` population, so they return
    /// `None` here rather than guessing.
    pub fn deref_ty(&self, ty: Ty) -> Option<Ty> {
        match self.ty_kind(ty) {
            TyKind::Ref(_, inner, _) => Some(*inner),
            TyKind::RawPtr(inner, _) => Some(*inner),
            _ => None,
        }
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

    /// Whether `adt_id` has an explicit `Drop` impl (or is a registered owning
    /// builtin). This is the single authority consulted by `needs_drop`; it
    /// replaces the previous per-crate guesses (`glyim-lower` hardcoded
    /// `String → true`, `glyim-opt` conservative `true` for unknown ADTs) which
    /// could disagree on identical types — the soundness risk flagged by the
    /// de-stubbing plan §8.2/§12.3.
    pub fn has_drop_impl(&self, adt_id: AdtId) -> bool {
        self.drop_impls.contains(&adt_id)
    }

    /// Authoritative `needs_drop` for the whole compiler.
    ///
    /// A type needs drop glue if it is `Copy`-adjacent (never needs drop), is a
    /// reference/raw-pointer (pointee is not owned), is a function pointer or
    /// function definition, or otherwise contains — transitively — a field that
    /// needs drop. Unions need drop (the user is responsible for the active
    /// variant). An `Adt` with a registered `Drop` impl needs drop regardless of
    /// its field types. Recursion is guarded by `visited` so self-referential
    /// types (e.g. `Box<Self>`) terminate; a cycle is treated as "needs drop" if
    /// any reachable field does, which is the correct answer for `Box<Self>`
    /// (the `Box` itself carries a `Drop` impl).
    ///
    /// Types the model cannot inspect (`dyn Trait`, generic parameters,
    /// projections, error/opaque types) return `false`: dropping them is
    /// impossible to do correctly without their concrete layout, and the
    /// alternative (spurious drop glue) is the worse failure mode. This matches
    /// the prior `glyim-lower` behaviour and is the safer default.
    pub fn needs_drop(&self, ty: Ty) -> bool {
        let mut visited = HashSet::new();
        self.needs_drop_rec(ty, &mut visited)
    }

    fn needs_drop_rec(&self, ty: Ty, visited: &mut HashSet<Ty>) -> bool {
        if self.is_copy(ty) {
            return false;
        }
        if !visited.insert(ty) {
            // Already exploring this type along the current path: a cycle.
            // Conservative `false` here; an enclosing field check may still
            // return `true` via an outer `Drop` impl or a non-cyclic field.
            return false;
        }
        let result = match self.ty_kind(ty) {
            TyKind::Ref(_, _, _) | TyKind::RawPtr(_, _) => false,
            // `String` is an owning builtin: it carries a destructor even though
            // it is represented as a standalone `TyKind` (no embedded `AdtId`),
            // so it must needs-drop. Mirrors the plan §8.2 intent of keying
            // drop-carrying-ness off the `Drop` lang item for owning types.
            TyKind::String => true,
            TyKind::FnPtr(_) | TyKind::FnDef(_, _) => false,
            TyKind::Slice(inner) | TyKind::Array(inner, _) => self.needs_drop_rec(*inner, visited),
            TyKind::Tuple(substs) => self
                .substitution_args(*substs)
                .iter()
                .any(|a| matches!(a, GenericArg::Ty(t) if self.needs_drop_rec(*t, visited))),
            TyKind::Closure(_, substs) => self
                .substitution_args(*substs)
                .iter()
                .any(|a| matches!(a, GenericArg::Ty(t) if self.needs_drop_rec(*t, visited))),
            TyKind::Adt(adt_id, _substs) => {
                if self.has_drop_impl(*adt_id) {
                    return true;
                }
                match self.adt_def(*adt_id) {
                    Some(adt) => {
                        if adt.kind == AdtKind::Union {
                            return true;
                        }
                        adt.variants.iter().any(|v| {
                            v.fields
                                .iter()
                                .any(|f| self.needs_drop_rec(f.ty, visited))
                        })
                    }
                    // Unregistered ADT: cannot inspect fields. Err toward no drop
                    // (see doc comment) rather than a spurious destructor.
                    None => false,
                }
            }
            _ => false,
        };
        visited.remove(&ty);
        result
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

    /// The value type of a constant definition (e.g. `i32` for
    /// `const X: i32 = ...`). Populated by typeck when checking the const.
    pub fn const_ty(&self, def_id: ConstDefId) -> Option<Ty> {
        self.const_tys.get(&def_id).copied()
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
    fn opaque_hidden_ty(&self, id: OpaqueTyId) -> Option<Ty> {
        self.opaque_hidden.get(&id).copied()
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
