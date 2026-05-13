use glyim_core::def_id::{TraitDefId, ImplDefId};
use crate::ty::Ty;
use crate::region::Region;
use crate::substitution::Substitution;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Predicate {
    Trait(TraitPredicate),
    RegionOutlives(RegionOutlivesPredicate),
    TypeOutlives(TypeOutlivesPredicate),
    WellFormed(Ty),
    Coerce(Ty, Ty),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraitPredicate {
    pub trait_ref: TraitRef,
    pub polarity: ImplPolarity,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraitRef {
    pub def_id: TraitDefId,
    pub substs: Substitution,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImplPolarity { Positive, Negative }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RegionOutlivesPredicate { pub a: Region, pub b: Region }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TypeOutlivesPredicate { pub ty: Ty, pub region: Region }
