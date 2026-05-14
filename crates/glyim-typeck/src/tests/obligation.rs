use crate::typeck_crate;
use glyim_core::arena::IndexVec;
use glyim_core::interner::Interner;
use glyim_core::primitives::*;
use glyim_hir::{
    Body, BodyId, CrateHir, Expr, ExprId, FnItem, Item, ItemId, ItemKind, Pat, PatId,
};
use glyim_span::Span;
use glyim_test::mock::MockSolver;
use super::test_utils::{empty_def_map, make_ty_ctx};

#[test]
fn obligation_collected() {
    // Placeholder: will test that obligations are collected
    // For now just compile check
}

#[test]
fn obligation_fulfilled() {
    // Placeholder
}
