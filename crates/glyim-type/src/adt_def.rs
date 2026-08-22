use crate::ty::{FieldIdx, Ty};
use glyim_core::arena::IndexVec;
use glyim_core::interner::Name;

#[derive(Clone, Debug)]
/// AdtDef.
pub struct AdtDef {
/// Struct.
    pub kind: AdtKind,
/// Struct.
    pub fields: IndexVec<FieldIdx, FieldDef>,
/// Struct.
    pub variants: Vec<VariantDef>,
    /// Names of the generic type parameters declared on the ADT
    /// (`struct S<T, U>`, `enum E<T>`). Empty for non-generic ADTs. Used to
    /// compute substitution arity when resolving type paths that carry generic
    /// arguments (unstub-5 P1.4). Stored as `Name`s (not the full HIR
    /// `GenericParam`) to avoid a `glyim-type` → `glyim-hir` dependency.
    pub generic_params: Vec<Name>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// AdtKind.
pub enum AdtKind {
/// Variant.
    Struct,
/// Variant.
    Enum,
/// Variant.
    Union,
}

#[derive(Clone, Debug)]
/// VariantDef.
pub struct VariantDef {
/// Struct.
    pub name: Name,
/// Struct.
    pub fields: IndexVec<FieldIdx, FieldDef>,
}

#[derive(Clone, Debug)]
/// FieldDef.
pub struct FieldDef {
/// Struct.
    pub name: Name,
/// Struct.
    pub ty: Ty,
}
