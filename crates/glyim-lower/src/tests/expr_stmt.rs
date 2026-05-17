//! Tests for expression statement not being dropped (U02-T02)
use glyim_test::mock::MockLowerCtx;
use glyim_test::test_frozen_ty_ctx;
use crate::lower_body;
use glyim_typeck::thir;
use glyim_core::arena::IndexVec;
use glyim_span::Span;

#[test]
fn expr_stmt_not_dropped() {
    let ctx = test_frozen_ty_ctx();
    let mock_ctx = MockLowerCtx::new(&ctx);
    let dummy_owner = glyim_core::def_id::DefId::new(
        glyim_core::def_id::CrateId::from_raw(0),
        glyim_core::def_id::LocalDefId::from_raw(0),
    );
    let thir_body = thir::Body {
        owner: dummy_owner,
        stmts: vec![],
        params: vec![],
        span: Span::DUMMY,
        return_ty: ctx.unit_ty(),
    };
    let result = lower_body(&mock_ctx, &thir_body);
    assert!(!result.diagnostics.iter().any(|d| d.is_error()));
}
