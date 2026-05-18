use super::{common::*, helpers::*};
use crate::Interpreter;
use glyim_core::{CrateId, DefId, LocalDefId, IntTy, FloatTy, UintTy};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{Ty, TyKind, Const, ConstKind, TyCtxMut, GenericArg};

fn mk_array_ty(tcx: &mut TyCtxMut, elem_ty: Ty, len: u64) -> Ty {
    let usize_ty = tcx.mk_ty(TyKind::Uint(UintTy::Usize));
    let const_len = Const {
        kind: ConstKind::Uint(len as u128),
        ty: usize_ty,
    };
    tcx.mk_ty(TyKind::Array(elem_ty, const_len))
}

#[test]
fn discriminant_returns_tag() {
    let mut tcx = glyim_test::test_ty_ctx();
    let int_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    // Correct conversion: GenericArg::Ty
    let tuple_substs = tcx.intern_substitution(vec![GenericArg::Ty(int_ty), GenericArg::Ty(int_ty)]);
    let tuple_ty = tcx.mk_ty(TyKind::Tuple(tuple_substs));
    let mut body = empty_body(Ty::UNIT);
    let local_enum = add_local(&mut body, tuple_ty, Mutability::Mut);
    let local_result = add_local(&mut body, int_ty, Mutability::Mut);
    let bb0 = BasicBlockIdx::from_raw(0);
    let agg = Rvalue::Aggregate(AggregateKind::Tuple, vec![const_int(42), const_int(0)]);
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local_enum), agg));
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local_result), Rvalue::Discriminant(Place::new(local_enum))));
    set_terminator(&mut body, bb0, TerminatorKind::Return);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.add_function(dummy_def_id(), body);
    let res = interp.run_body(&interp.function_table.values().next().unwrap().clone());
    assert!(res.is_ok());
    let val = interp.get_local_value(local_result).unwrap();
    assert_eq!(val, &InterpValue::Int(42));
}

#[test]
fn cast_int_to_float() {
    let mut tcx = glyim_test::test_ty_ctx();
    let float_ty = tcx.mk_ty(TyKind::Float(FloatTy::F64));
    let mut body = empty_body(Ty::UNIT);
    let local = add_local(&mut body, float_ty, Mutability::Mut);
    let bb0 = BasicBlockIdx::from_raw(0);
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local), Rvalue::Cast(CastKind::IntToFloat, const_int(42), float_ty)));
    set_terminator(&mut body, bb0, TerminatorKind::Return);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.add_function(dummy_def_id(), body);
    let res = interp.run_body(&interp.function_table.values().next().unwrap().clone());
    assert!(res.is_ok());
    let val = interp.get_local_value(local).unwrap();
    assert_eq!(val, &InterpValue::Float(42.0));
}

#[test]
fn cast_float_to_int() {
    let mut tcx = glyim_test::test_ty_ctx();
    let int_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = empty_body(Ty::UNIT);
    let local = add_local(&mut body, int_ty, Mutability::Mut);
    let bb0 = BasicBlockIdx::from_raw(0);
    let float_const = MirConst {
        kind: MirConstKind::FloatBits(123.456_f64.to_bits()),
        ty: tcx.mk_ty(TyKind::Float(FloatTy::F64)),
        span: Span::DUMMY,
    };
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local), Rvalue::Cast(CastKind::FloatToInt, Operand::Constant(float_const), int_ty)));
    set_terminator(&mut body, bb0, TerminatorKind::Return);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.add_function(dummy_def_id(), body);
    let res = interp.run_body(&interp.function_table.values().next().unwrap().clone());
    assert!(res.is_ok());
    let val = interp.get_local_value(local).unwrap();
    assert_eq!(val, &InterpValue::Int(123));
}

#[test]
fn repeat_creates_array() {
    let mut tcx = glyim_test::test_ty_ctx();
    let elem_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let array_ty = mk_array_ty(&mut tcx, elem_ty, 5);
    let mut body = empty_body(Ty::UNIT);
    let local = add_local(&mut body, array_ty, Mutability::Mut);
    let bb0 = BasicBlockIdx::from_raw(0);
    // const_int returns Operand, which is correct for first argument.
    // Second argument must be MirConst (mir_const_usize returns MirConst)
    let repeat = Rvalue::Repeat(const_int(42), mir_const_usize(5));
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local), repeat));
    set_terminator(&mut body, bb0, TerminatorKind::Return);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.add_function(dummy_def_id(), body);
    let res = interp.run_body(&interp.function_table.values().next().unwrap().clone());
    assert!(res.is_ok());
    let val = interp.get_local_value(local).unwrap();
    let expected = InterpValue::Aggregate(vec![InterpValue::Int(42); 5]);
    assert_eq!(val, &expected);
}

#[test]
fn len_of_array() {
    let mut tcx = glyim_test::test_ty_ctx();
    let elem_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let array_ty = mk_array_ty(&mut tcx, elem_ty, 7);
    let mut body = empty_body(Ty::UNIT);
    let local_array = add_local(&mut body, array_ty, Mutability::Mut);
    let local_len = add_local(&mut body, tcx.mk_ty(TyKind::Uint(UintTy::Usize)), Mutability::Mut);
    let bb0 = BasicBlockIdx::from_raw(0);
    let init = Rvalue::Repeat(const_int(0), mir_const_usize(7));
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local_array), init));
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local_len), Rvalue::Len(Place::new(local_array))));
    set_terminator(&mut body, bb0, TerminatorKind::Return);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.add_function(dummy_def_id(), body);
    let res = interp.run_body(&interp.function_table.values().next().unwrap().clone());
    assert!(res.is_ok());
    let val = interp.get_local_value(local_len).unwrap();
    assert_eq!(val, &InterpValue::Int(7));
}

#[test]
fn fn_constant_used_as_operand() {
    let mut tcx = glyim_test::test_ty_ctx();
    let fn_def_id = glyim_core::def_id::FnDefId::from_raw(100);
    let fn_ty = tcx.mk_ty(TyKind::FnDef(fn_def_id, glyim_type::Substitution::empty()));
    let mut body = empty_body(Ty::UNIT);
    let local = add_local(&mut body, fn_ty, Mutability::Mut);
    let bb0 = BasicBlockIdx::from_raw(0);
    let const_val = MirConst {
        kind: MirConstKind::Fn(fn_def_id, glyim_type::Substitution::empty()),
        ty: fn_ty,
        span: Span::DUMMY,
    };
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local), Rvalue::Use(Operand::Constant(const_val))));
    set_terminator(&mut body, bb0, TerminatorKind::Return);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.add_function(dummy_def_id(), body);
    let res = interp.run_body(&interp.function_table.values().next().unwrap().clone());
    assert!(res.is_ok());
    let val = interp.get_local_value(local).unwrap();
    let expected_def_id = DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(fn_def_id.to_raw()));
    assert_eq!(val, &InterpValue::Fn(expected_def_id));
}
