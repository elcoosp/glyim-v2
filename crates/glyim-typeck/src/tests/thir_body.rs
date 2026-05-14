use crate::typeck_crate;
use glyim_core::arena::IndexVec;
use glyim_core::interner::Interner;
use glyim_hir::{
    Body, BodyId, CrateHir, Expr, ExprId, FnItem, Item, ItemId, ItemKind, Pat, PatId,
};
use glyim_span::Span;
use glyim_test::mock::MockSolver;
use super::test_utils::{empty_def_map, make_ty_ctx};

#[test]
fn thir_body_constructed() {
    // Placeholder
}
