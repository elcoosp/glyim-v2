use super::common::*;
use crate::*;

#[test]
fn interpret_unreachable_panics() {
    let tcx = glyim_test::test_frozen_ty_ctx();
    let body = build_unreachable_body();
    let mut interp = Interpreter::new(&tcx);
    let res = interp.run_body(&body);
    assert!(matches!(res, Err(InterpError::Panic(_))));
}
