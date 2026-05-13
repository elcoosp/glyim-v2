use super::common::*;
use crate::*;
use glyim_mir::LocalIdx;
use glyim_test::test_ty_ctx;
use glyim_type::{IntTy, TyKind};

#[test]
fn interpret_integer_add() {
    let tcx = test_ty_ctx().freeze();
    let i32_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let body = build_add_body(&tcx, 3, 4, i32_ty);
    let mut interp = Interpreter::new(tcx.clone());
    interp.run_body(&body).unwrap();
    let val = interp.get_local_value(LocalIdx::from_raw(1)).unwrap();
    assert_eq!(val, &InterpValue::Int(7));
}

#[test]
fn interpret_integer_sub() {
    let tcx = test_ty_ctx().freeze();
    let i32_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let body = build_sub_body(&tcx, 10, 3, i32_ty);
    let mut interp = Interpreter::new(tcx.clone());
    interp.run_body(&body).unwrap();
    let val = interp.get_local_value(LocalIdx::from_raw(1)).unwrap();
    assert_eq!(val, &InterpValue::Int(7));
}
