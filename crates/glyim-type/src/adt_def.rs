use crate::ty::{FieldIdx, Ty};
use glyim_core::arena::IndexVec;
use glyim_core::interner::Name;

#[derive(Clone, Debug)]
pub struct AdtDef {
    pub kind: AdtKind,
    pub fields: IndexVec<FieldIdx, FieldDef>,
    pub variants: Vec<VariantDef>,
    /// Names of the generic type parameters declared on the ADT
    /// (`struct S<T, U>`, `enum E<T>`). Empty for non-generic ADTs. Used to
    /// compute substitution arity when resolving type paths that carry generic
    /// arguments (unstub-5 P1.4). Stored as `Name`s (not the full HIR
    /// `GenericParam`) to avoid a `glyim-type` → `glyim-hir` dependency.
    pub generic_params: Vec<Name>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdtKind {
    Struct,
    Enum,
    Union,
}

#[derive(Clone, Debug)]
pub struct VariantDef {
    pub name: Name,
    pub fields: IndexVec<FieldIdx, FieldDef>,
}

#[derive(Clone, Debug)]
pub struct FieldDef {
    pub name: Name,
    pub ty: Ty,
}
