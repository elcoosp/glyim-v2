use glyim_span::Span;

use crate::{Path, TypeRef};

/// A single where clause bound, e.g. `T: Clone + Copy`
#[derive(Clone, Debug)]
pub struct WhereClause {
    pub ty: TypeRef,
    pub bounds: Vec<TraitBound>,
    pub span: Span,
}

/// A trait bound, e.g. `Clone`
#[derive(Clone, Debug)]
pub struct TraitBound {
    pub trait_path: Path,
    pub span: Span,
}
