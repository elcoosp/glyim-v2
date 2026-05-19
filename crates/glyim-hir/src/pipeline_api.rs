use glyim_core::interner::Interner;
use glyim_diag::GlyimDiagnostic;
use glyim_syntax::SyntaxNode;

use super::CrateHir;
use super::lower::lower_crate;

/// Build the HIR from the AST, returning both the HIR and any diagnostics collected during lowering.
pub fn lower_crate_for_pipeline(
    root: &SyntaxNode,
    interner: &mut Interner,
) -> (CrateHir, Vec<GlyimDiagnostic>) {
    let mut diags = Vec::new();
    let hir = lower_crate(root, interner, &mut diags);
    (hir, diags)
}
