use super::common::*;
use crate::*;

#[test]
fn interpret_branch_on_true() {
    let body = build_branch_body(true, false, true);
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut interp = Interpreter::new(tcx);
    interp.run_body(&body).unwrap();
}

#[test]
fn interpret_branch_on_false() {
    let body = build_branch_body(false, true, false);
    let tcx = glyim_test::test_frozen_ty_ctx();
    let mut interp = Interpreter::new(tcx);
    interp.run_body(&body).unwrap();
}
