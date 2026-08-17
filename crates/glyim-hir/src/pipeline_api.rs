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

#[cfg(test)]
mod tests {
    use super::*;
    use glyim_frontend::parse_to_syntax;
    use glyim_span::FileId;
    use crate::ItemKind;

    /// Plan §6.1: `const fn` must lower to an `FnItem` with `is_const == true`,
    /// while a plain `fn` stays `is_const == false`.
    fn lower_first_fn(src: &str) -> bool {
        let root = parse_to_syntax(src, FileId::BOGUS).root;
        let mut interner = Interner::new();
        let (hir, _) = lower_crate_for_pipeline(&root, &mut interner);
        let item = hir.items.iter().next().expect("expected at least one item");
        match &item.kind {
            ItemKind::Fn(f) => f.is_const,
            _ => panic!("expected an Fn item"),
        }
    }

    #[test]
    fn const_fn_lowers_with_is_const_true() {
        assert!(lower_first_fn("const fn f() { let x = 1; }"));
    }

    #[test]
    fn plain_fn_lowers_with_is_const_false() {
        assert!(!lower_first_fn("fn f() { let x = 1; }"));
    }
}
