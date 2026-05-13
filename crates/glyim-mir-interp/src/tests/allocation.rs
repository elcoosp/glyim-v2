use super::common::*;
use crate::*;
use glyim_mir::LocalIdx;
use glyim_test::test_ty_ctx;
use glyim_type::{IntTy, TyKind};

#[test]
fn interpret_allocate_write_read() {
    let tcx = test_ty_ctx().freeze();
    let val = 99;
    let body = build_allocation_body(&tcx, val);
    let mut interp = Interpreter::new(tcx.clone());
    interp.run_body(&body).unwrap();
    let stored = interp.get_local_value(LocalIdx::from_raw(1)).unwrap();
    assert_eq!(stored, &InterpValue::Int(val));
}
