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
    fn lower_first_fn_const(src: &str) -> bool {
        let root = parse_to_syntax(src, FileId::BOGUS).root;
        let mut interner = Interner::new();
        let (hir, _) = lower_crate_for_pipeline(&root, &mut interner);
        let item = hir.items.iter().next().expect("expected at least one item");
        match &item.kind {
            ItemKind::Fn(f) => f.is_const,
            _ => panic!("expected an Fn item"),
        }
    }

    /// Plan §6.1: `async fn` must lower to an `FnItem` with `is_async == true`,
    /// while a plain `fn` stays `is_async == false`. The async desugaring
    /// itself is a larger feature (Future lang item + state machine); this test
    /// locks in the keyword → `FnItem.is_async` plumbing, matching the
    /// `is_const` pattern.
    fn lower_first_fn_async(src: &str) -> bool {
        let root = parse_to_syntax(src, FileId::BOGUS).root;
        let mut interner = Interner::new();
        let (hir, _) = lower_crate_for_pipeline(&root, &mut interner);
        let item = hir.items.iter().next().expect("expected at least one item");
        match &item.kind {
            ItemKind::Fn(f) => f.is_async,
            _ => panic!("expected an Fn item"),
        }
    }

    #[test]
    fn const_fn_lowers_with_is_const_true() {
        assert!(lower_first_fn_const("const fn f() { let x = 1; }"));
    }

    #[test]
    fn plain_fn_lowers_with_is_const_false() {
        assert!(!lower_first_fn_const("fn f() { let x = 1; }"));
    }

    #[test]
    fn async_fn_lowers_with_is_async_true() {
        assert!(lower_first_fn_async("async fn f() { let x = 1; }"));
    }

    #[test]
    fn plain_fn_lowers_with_is_async_false() {
        assert!(!lower_first_fn_async("fn f() { let x = 1; }"));
    }

    #[test]
    fn async_const_fn_lowers_both_flags() {
        // `async` must not be swallowed by the `const` detection; both flags
        // should be independently populated (here `const` absent, `async` set).
        assert!(lower_first_fn_async("async fn f() { let x = 1; }"));
        assert!(!lower_first_fn_const("async fn f() { let x = 1; }"));
    }
}
