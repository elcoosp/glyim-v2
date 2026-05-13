use glyim_core::def_id::*;
use glyim_core::interner::Name;
use glyim_core::primitives::*;
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ty { raw: u32 }

impl Ty {
    #[inline]
    pub(crate) const fn from_raw(raw: u32) -> Self { Self { raw } }
    #[inline] pub fn to_raw(self) -> u32 { self.raw }
    #[inline] pub fn index(self) -> usize { self.raw as usize }

    pub const ERROR: Ty = Ty::from_raw(0);
    pub const NEVER: Ty = Ty::from_raw(1);
    pub const UNIT: Ty = Ty::from_raw(2);
    pub const BOOL: Ty = Ty::from_raw(3);
}

impl fmt::Debug for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Ty({})", self.raw) }
}

glyim_core::define_idx!(TyVar);
glyim_core::define_idx!(IntVar);
glyim_core::define_idx!(FloatVar);
glyim_core::define_idx!(RegionVid);
glyim_core::define_idx!(ConstVar);
glyim_core::define_idx!(FieldIdx);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UniverseIndex(pub u32);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TyKind {
    Never, Unit, Bool, Int(IntTy), Uint(UintTy), Float(FloatTy), Char, String,
    Infer(InferVar), Adt(AdtId, Substitution), FnDef(FnDefId, Substitution),
    Closure(ClosureId, Substitution), FnPtr(FnSig), Ref(Region, Ty, Mutability),
    RawPtr(Ty, Mutability), Slice(Ty), Array(Ty, Const), Tuple(Substitution),
    Dynamic(Binder<Box<[Predicate]>>, Region), Opaque(OpaqueTyId, Substitution),
    Param(ParamTy), Bound(u32, BoundTy), Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InferVar { Ty(TyVar), Int(IntVar), Float(FloatVar) }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParamTy { pub index: u32, pub name: Name }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BoundTy { pub var: u32, pub kind: BoundTyKind }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BoundTyKind { Anon, Param(Name) }

// Substitution with pub(crate) fields
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Substitution {
    pub(crate) index: u32,
    pub(crate) len: u16,
}
impl Substitution {
    pub fn is_empty(self) -> bool { self.len == 0 }
    pub fn index(self) -> u32 { self.index }
    pub fn len(self) -> u16 { self.len }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Predicate { Trait(TraitPredicate) }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraitPredicate { pub trait_ref: TraitRef }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraitRef { pub def_id: TraitDefId, pub substs: Substitution }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FnSig { pub inputs: Substitution, pub output: Ty }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Region { Static, Var(RegionVid), Error }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Const { pub kind: ConstKind, pub ty: Ty }
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ConstKind { Int(i128), Unit, Error }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Binder<T> { pub value: T, pub bound_vars: Box<[BoundVariableKind]> }
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BoundVariableKind { Ty(BoundTyKind), Region(BoundRegionKind), Const }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BoundRegionKind { BrAnon(u32), BrNamed(Name), BrEnv }
