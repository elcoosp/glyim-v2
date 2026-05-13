use glyim_core::interner::Name;
use glyim_span::{Span, HygieneCtx};
use glyim_syntax::SyntaxNode;
use glyim_diag::GlyimDiagnostic;

pub struct Expander<'a> {
    hygiene: &'a mut HygieneCtx,
    macros: Vec<MacroDef>,
}

#[derive(Clone, Debug)]
pub struct MacroDef {
    pub name: Name,
    pub span: Span,
}

impl<'a> Expander<'a> {
    pub fn new(hygiene: &'a mut HygieneCtx) -> Self {
        Self { hygiene, macros: Vec::new() }
    }

    pub fn expand_crate(&mut self, root: &SyntaxNode) -> (SyntaxNode, Vec<GlyimDiagnostic>) {
        (root.clone(), Vec::new())
    }
}
