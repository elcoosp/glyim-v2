//! Tests for Stream W2-C03: HIR lowering of index expressions and new patterns.
//!
//! Verifies that:
//! - `arr[0]` lowers to `Expr::Index`
//! - `0 | 1` lowers to `Pat::Or`
//! - `0..=5` lowers to `Pat::Range` inclusive
//! - `[a, b]` lowers to `Pat::Slice`

use glyim_core::interner::Interner;
use glyim_frontend::parse_to_syntax;
use glyim_span::FileId;

use crate::{BodyId, CrateHir, Expr, Pat};

/// Parse source code and lower it to HIR, returning the CrateHir and any
/// diagnostics collected during lowering.
fn parse_and_lower(source: &str) -> (CrateHir, Vec<glyim_diag::GlyimDiagnostic>) {
    let result = parse_to_syntax(source, FileId::from_raw(1));
    let mut interner = Interner::new();
    crate::pipeline_api::lower_crate_for_pipeline(&result.root, &mut interner)
}

/// Return the first body in the HIR, or panic if none exists.
fn first_body(hir: &CrateHir) -> &crate::Body {
    if hir.bodies.is_empty() {
        panic!("expected at least one body in HIR, found none");
    }
    &hir.bodies[BodyId::from_raw(0)]
}

/// W2-C03-T01: `arr[0]` lowers to `Expr::Index`
#[test]
fn index_expr() {
    let (hir, _diags) = parse_and_lower("fn main() { arr[0] }");
    let body = first_body(&hir);
    let found = body.exprs.iter().any(|e| matches!(e, Expr::Index { .. }));
    assert!(found, "expected Expr::Index in lowered HIR for arr[0]");
}

/// W2-C03-T02: `0 | 1` lowers to `Pat::Or`
#[test]
fn pat_or() {
    let (hir, _diags) = parse_and_lower("fn main() { match x { 0 | 1 => y } }");
    let body = first_body(&hir);
    let found = body.pats.iter().any(|p| matches!(p, Pat::Or(_)));
    assert!(found, "expected Pat::Or in lowered HIR for 0 | 1");
}

/// W2-C03-T03: `0..=5` lowers to `Pat::Range` inclusive
#[test]
fn pat_range_inclusive() {
    let (hir, _diags) = parse_and_lower("fn main() { match x { 0..=5 => y } }");
    let body = first_body(&hir);
    let found = body
        .pats
        .iter()
        .any(|p| matches!(p, Pat::Range { inclusive: true, .. }));
    assert!(
        found,
        "expected Pat::Range {{ inclusive: true }} in lowered HIR for 0..=5"
    );
}

/// W2-C03-T04: `[a, b]` lowers to `Pat::Slice`
#[test]
fn pat_slice() {
    let (hir, _diags) = parse_and_lower("fn main() { match x { [a, b] => y } }");
    let body = first_body(&hir);
    let found = body.pats.iter().any(|p| matches!(p, Pat::Slice(_)));
    assert!(found, "expected Pat::Slice in lowered HIR for [a, b]");
}
