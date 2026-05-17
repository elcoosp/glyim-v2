use crate::lower_body;
use crate::tests::support::MockLowerCtx;
use glyim_test::test_frozen_ty_ctx;
use glyim_typeck::thir;
use glyim_span::Span;
use glyim_core::def_id::{CrateId, LocalDefId, DefId};

#[test]
fn expr_stmt_works() {
    let ctx = test_frozen_ty_ctx();
    let mock_ctx = MockLowerCtx::new(&ctx);
    let owner = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0));
    let thir_body = thir::Body {
        owner,
        stmts: vec![],
        params: vec![],
        span: Span::DUMMY,
        return_ty: ctx.unit_ty(),
    };
    let result = lower_body(&mock_ctx, &thir_body);
    assert!(!result.diagnostics.iter().any(|d| d.is_error()));
}
