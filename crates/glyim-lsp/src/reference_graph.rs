use glyim_span::{FileId, Span};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Reference {
    pub file_id: FileId,
    pub span: Span,
    pub is_definition: bool,
    pub kind: ReferenceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind { Call, TypeReference, FieldAccess, Constructor, Pattern }

pub struct ReferenceGraph {
    references: HashMap<String, Vec<Reference>>,
}

impl Default for ReferenceGraph {
    fn default() -> Self { Self::new() }
}

impl ReferenceGraph {
    pub fn new() -> Self { Self { references: HashMap::new() } }

    pub fn build_from_hir(&mut self, _file_id: FileId, _hir: &glyim_hir::CrateHir, _interner: &glyim_core::Interner) {
        tracing::warn!("STUB: ReferenceGraph::build_from_hir");
    }

    pub fn find_references(&self, symbol_name: &str) -> &[Reference] {
        self.references.get(symbol_name).map(|v| v.as_slice()).unwrap_or(&[])
    }

    #[doc(hidden)]
    pub fn insert_test_reference(&mut self, name: &str, reference: Reference) {
        self.references.entry(name.to_string()).or_default().push(reference);
    }
}
