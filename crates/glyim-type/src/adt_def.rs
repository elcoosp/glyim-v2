use crate::ty::{FieldIdx, Ty};
use glyim_core::arena::IndexVec;
use glyim_core::interner::Name;

#[derive(Clone, Debug)]
pub struct AdtDef {
    pub kind: AdtKind,
    pub fields: IndexVec<FieldIdx, FieldDef>,
    pub variants: Vec<VariantDef>,
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
