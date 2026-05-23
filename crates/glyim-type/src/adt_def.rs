use crate::ty::Ty;
use crate::FieldIdx;
use glyim_core::IndexVec;

#[derive(Debug, Clone)]
pub(crate) struct AdtDef {
    /// Flattened fields (for layout compatibility)
    pub fields: IndexVec<FieldIdx, FieldDef>,
    pub kind: AdtKind,
    /// For enums: list of field types for each variant.
    /// For structs/unions: a single variant containing all fields.
    pub variant_fields: Vec<Vec<Ty>>,
    /// Precomputed type for each variant (unit, single, or tuple).
    pub variant_tys: Vec<Ty>,
}

impl AdtDef {
    pub(crate) fn new(
        fields: IndexVec<FieldIdx, FieldDef>,
        kind: AdtKind,
        variant_fields: Vec<Vec<Ty>>,
        variant_tys: Vec<Ty>,
    ) -> Self {
        Self {
            fields,
            kind,
            variant_fields,
            variant_tys,
        }
    }

    pub(crate) fn variant_type(&self, idx: u32) -> Ty {
        self.variant_tys.get(idx as usize).copied().unwrap_or(Ty::ERROR)
    }
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub ty: Ty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdtKind {
    Struct,
    Enum,
    Union,
}
