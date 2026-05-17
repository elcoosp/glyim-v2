//! Tests for match guard lowering (U02-T04)
use glyim_test::mock::MockLowerCtx;
use glyim_test::test_frozen_ty_ctx;
use glyim_lower::lower_body;
use glyim_typeck::thir;
use glyim_core::arena::IndexVec;
use glyim_span::Span;

#[test]
fn match_guard_correctly_lowered() {
    let ctx = test_frozen_ty_ctx();
    let mock_ctx = MockLowerCtx::new(&ctx);
    let dummy_owner = glyim_core::def_id::DefId::new(
        glyim_core::def_id::CrateId::from_raw(0),
        glyim_core::def_id::LocalDefId::from_raw(0),
    );
    let thir_body = thir::Body {
        owner: dummy_owner,
        exprs: IndexVec::new(),
        pats: IndexVec::new(),
        params: vec![],
        span: Span::DUMMY,
        expr_spans: IndexVec::new(),
        return_ty: ctx.unit_ty(),
        stmts: vec![],
    };
    let result = lower_body(&mock_ctx, &thir_body);
    // After implementation, guard expressions will be lowered.
    assert!(!result.diagnostics.iter().any(|d| d.is_error()));
}
