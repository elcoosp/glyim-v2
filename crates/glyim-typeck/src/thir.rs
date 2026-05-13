//! Typed High-Level IR (THIR) stub.
//! For v0.1.0, we define a minimal Body type to satisfy the lower crate.

use glyim_core::def_id::DefId;
use glyim_span::Span;

#[derive(Clone, Debug)]
pub struct Body {
    pub owner: DefId,
    pub span: Span,
    // In the future, add statements, expressions, etc.
}

impl Body {
    pub fn dummy(owner: DefId) -> Self {
        Self { owner, span: Span::DUMMY }
    }
}
