use crate::display::TypeLookup;
use crate::substitution::GenericArg;
use crate::ty::*;
use glyim_core::def_id::AdtId;
use glyim_core::primitives::Mutability;
use std::collections::{HashMap, HashSet};

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct AutoTraitFlags: u8 {
        const SEND  = 1 << 0;
        const SYNC  = 1 << 1;
        const UNPIN = 1 << 2;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AutoTrait {
    Send,
    Sync,
    Unpin,
}

impl AutoTrait {
    pub fn flag(self) -> AutoTraitFlags {
        match self {
            AutoTrait::Send => AutoTraitFlags::SEND,
            AutoTrait::Sync => AutoTraitFlags::SYNC,
            AutoTrait::Unpin => AutoTraitFlags::UNPIN,
        }
    }

    pub const ALL: [AutoTrait; 3] = [AutoTrait::Send, AutoTrait::Sync, AutoTrait::Unpin];
}

#[derive(Clone, Debug, Default)]
pub struct AdtRepr {
    pub field_tys: Vec<Ty>,
}

impl AdtRepr {
    pub fn new(field_tys: Vec<Ty>) -> Self {
        Self { field_tys }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AutoTraitRegistry {
    negative_impls: HashSet<(AdtId, AutoTrait)>,
    manual_impls: HashSet<(AdtId, AutoTrait)>,
}

impl AutoTraitRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_negative_impl(&mut self, adt_id: AdtId, auto_trait: AutoTrait) {
        self.negative_impls.insert((adt_id, auto_trait));
    }

    pub fn register_manual_impl(&mut self, adt_id: AdtId, auto_trait: AutoTrait) {
        self.manual_impls.insert((adt_id, auto_trait));
    }

    pub fn has_negative_impl(&self, adt_id: AdtId, auto_trait: AutoTrait) -> bool {
        self.negative_impls.contains(&(adt_id, auto_trait))
    }

    pub fn has_manual_impl(&self, adt_id: AdtId, auto_trait: AutoTrait) -> bool {
        self.manual_impls.contains(&(adt_id, auto_trait))
    }
}

/// Compute auto trait flags for a type.
///
/// Uses coinductive semantics: recursive types are assumed to implement
/// all auto traits until proven otherwise.
pub fn compute_auto_traits(
    ty: Ty,
    lookup: &dyn TypeLookup,
    registry: &AutoTraitRegistry,
    adt_reprs: &HashMap<AdtId, AdtRepr>,
) -> AutoTraitFlags {
    let mut cache = HashMap::new();
    let mut evaluating = HashSet::new();
    compute_auto_traits_recursive(ty, lookup, registry, adt_reprs, &mut cache, &mut evaluating)
}

fn compute_auto_traits_recursive(
    ty: Ty,
    lookup: &dyn TypeLookup,
    registry: &AutoTraitRegistry,
    adt_reprs: &HashMap<AdtId, AdtRepr>,
    cache: &mut HashMap<Ty, AutoTraitFlags>,
    evaluating: &mut HashSet<Ty>,
) -> AutoTraitFlags {
    if let Some(&flags) = cache.get(&ty) {
        return flags;
    }

    if evaluating.contains(&ty) {
        return AutoTraitFlags::all();
    }

    evaluating.insert(ty);

    let flags = compute_auto_traits_for_kind(ty, lookup, registry, adt_reprs, cache, evaluating);

    evaluating.remove(&ty);
    cache.insert(ty, flags);
    flags
}

fn compute_auto_traits_for_kind(
    ty: Ty,
    lookup: &dyn TypeLookup,
    registry: &AutoTraitRegistry,
    adt_reprs: &HashMap<AdtId, AdtRepr>,
    cache: &mut HashMap<Ty, AutoTraitFlags>,
    evaluating: &mut HashSet<Ty>,
) -> AutoTraitFlags {
    match lookup.ty_kind(ty) {
        TyKind::Bool
        | TyKind::Int(_)
        | TyKind::Uint(_)
        | TyKind::Float(_)
        | TyKind::Char
        | TyKind::Never
        | TyKind::Unit
        | TyKind::String => AutoTraitFlags::all(),

        TyKind::Ref(_, inner, Mutability::Not) => {
            let inner_flags = compute_auto_traits_recursive(
                *inner, lookup, registry, adt_reprs, cache, evaluating,
            );
            let mut flags = AutoTraitFlags::UNPIN;
            if inner_flags.contains(AutoTraitFlags::SYNC) {
                flags |= AutoTraitFlags::SEND | AutoTraitFlags::SYNC;
            }
            flags
        }

        TyKind::Ref(_, inner, Mutability::Mut) => {
            let inner_flags = compute_auto_traits_recursive(
                *inner, lookup, registry, adt_reprs, cache, evaluating,
            );
            let mut flags = AutoTraitFlags::UNPIN;
            if inner_flags.contains(AutoTraitFlags::SEND) {
                flags |= AutoTraitFlags::SEND;
            }
            if inner_flags.contains(AutoTraitFlags::SYNC) {
                flags |= AutoTraitFlags::SYNC;
            }
            flags
        }

        TyKind::RawPtr(_, _) => AutoTraitFlags::UNPIN,

        TyKind::Slice(inner) => {
            compute_auto_traits_recursive(*inner, lookup, registry, adt_reprs, cache, evaluating)
        }

        TyKind::Array(inner, _) => {
            compute_auto_traits_recursive(*inner, lookup, registry, adt_reprs, cache, evaluating)
        }

        TyKind::Tuple(substs) => {
            let mut flags = AutoTraitFlags::all();
            for arg in lookup.substitution_args(*substs) {
                if let GenericArg::Ty(t) = arg {
                    flags &= compute_auto_traits_recursive(
                        *t, lookup, registry, adt_reprs, cache, evaluating,
                    );
                }
            }
            flags
        }

        TyKind::Adt(adt_id, _substs) => {
            let mut flags = AutoTraitFlags::all();

            for auto_trait in AutoTrait::ALL {
                let trait_flag = auto_trait.flag();

                if registry.has_negative_impl(*adt_id, auto_trait) {
                    flags -= trait_flag;
                    continue;
                }

                if registry.has_manual_impl(*adt_id, auto_trait) {
                    continue;
                }

                if let Some(repr) = adt_reprs.get(adt_id) {
                    for &field_ty in &repr.field_tys {
                        let field_flags = compute_auto_traits_recursive(
                            field_ty, lookup, registry, adt_reprs, cache, evaluating,
                        );
                        if !field_flags.contains(trait_flag) {
                            flags -= trait_flag;
                            break;
                        }
                    }
                } else {
                    tracing::warn!(
                        "STUB: no AdtRepr registered for AdtId {}, assuming no auto traits",
                        adt_id.to_raw()
                    );
                    flags = AutoTraitFlags::empty();
                    break;
                }
            }

            flags
        }

        TyKind::FnPtr(_) | TyKind::FnDef(_, _) => AutoTraitFlags::all(),

        TyKind::Closure(_, substs) => {
            let mut flags = AutoTraitFlags::all();
            for arg in lookup.substitution_args(*substs) {
                if let GenericArg::Ty(t) = arg {
                    let inner = compute_auto_traits_recursive(
                        *t, lookup, registry, adt_reprs, cache, evaluating,
                    );
                    flags &= inner;
                }
            }
            flags
        }

        TyKind::Dynamic(_, _) => AutoTraitFlags::empty(),

        TyKind::Opaque(_, _) | TyKind::Projection(_) => AutoTraitFlags::empty(),

        TyKind::Infer(_) | TyKind::Param(_) | TyKind::Bound(_, _) | TyKind::Error => {
            AutoTraitFlags::empty()
        }
    }
}
