use glyim_core::interner::Interner;
use glyim_syntax::SyntaxNode;

use super::lower::lower_crate;
use super::CrateHir;

/// Build the HIR from the AST. This is a public wrapper around the crate-internal lower_crate.
pub fn lower_crate_for_pipeline(root: &SyntaxNode, interner: &mut Interner) -> CrateHir {
    lower_crate(root, interner)
}
