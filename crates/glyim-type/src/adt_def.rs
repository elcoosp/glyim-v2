//! ADT (struct/enum/union) definition storage for type context.

use crate::ty::Ty;

#[derive(Debug, Clone)]
pub(crate) struct AdtDef {
    /// For structs/unions: the list of field types.
    /// For enums: flattened list of all fields across all variants (legacy).
    pub fields: Vec<Ty>,
    pub kind: AdtKind,
    /// Precomputed type for each variant (indexed by variant index).
    /// For structs/unions, length = 1.
    pub variant_tys: Vec<Ty>,
}

impl AdtDef {
    pub(crate) fn new(fields: Vec<Ty>, kind: AdtKind, variant_tys: Vec<Ty>) -> Self {
        Self {
            fields,
            kind,
            variant_tys,
        }
    }

    /// Returns the type of a variant by its raw index.
    pub(crate) fn variant_type(&self, idx: u32) -> Ty {
        self.variant_tys.get(idx as usize).copied().unwrap_or(Ty::ERROR)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdtKind {
    Struct,
    Enum,
    Union,
}
