use glyim_span::Span;

use crate::{Path, TypeRef};

/// A single where clause bound, e.g. `T: Clone + Copy`
#[derive(Clone, Debug)]
pub struct WhereClause {
/// Struct.
    pub ty: TypeRef,
/// Struct.
    pub bounds: Vec<TraitBound>,
/// Struct.
    pub span: Span,
}

/// A trait bound, e.g. `Clone`
#[derive(Clone, Debug)]
pub struct TraitBound {
/// Struct.
    pub trait_path: Path,
/// Struct.
    pub span: Span,
}
