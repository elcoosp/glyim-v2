//! Regression tests for de-stubbing plan §11.2: `eval_binary_op::Rem`/`Div`
//! must use `checked_*` semantics — division/remainder by zero is a clean
//! `InterpError::DivisionByZero` (not a Rust panic that aborts the compiler),
//! and signed `MIN % -1` / `MIN / -1` yields `0` (the value `checked_rem`/
//! `checked_div` cannot represent).

use crate::*;
use glyim_core::{BinOp, CrateId, DefId, IndexVec, IntTy, LocalDefId, Mutability, UintTy};
use glyim_span::Span;
use glyim_test::test_ty_ctx;
use glyim_type::{Ty, TyCtxMut, TyKind};

fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

fn local_decl(ty: Ty, mutability: Mutability) -> LocalDecl {
    LocalDecl {
        ty,
        mutability,
        source_info: SourceInfo::new(Span::DUMMY),
    }
}

/// Build a signed-integer binary-op body (matching `advanced::build_binop_body`).
fn build_binop_body_signed(tcx: &mut TyCtxMut, op: BinOp, lhs: i128, rhs: i128) -> Body {
    let ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    let res_local = LocalIdx::from_raw(1);
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(ty, Mutability::Mut),
    ]);
    let c1 = MirConst {
        kind: MirConstKind::Int(lhs),
        ty,
        span: Span::DUMMY,
    };
    let c2 = MirConst {
        kind: MirConstKind::Int(rhs),
        ty,
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(res_local),
                Rvalue::BinaryOp(op, Box::new((Operand::Constant(c1), Operand::Constant(c2)))),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    body
}

/// Build an unsigned-integer binary-op body.
fn build_binop_body_uint(tcx: &mut TyCtxMut, op: BinOp, lhs: u128, rhs: u128) -> Body {
    let ty = tcx.mk_ty(TyKind::Uint(glyim_core::UintTy::U32));
    let mut body = Body::dummy(dummy_def_id());
    let res_local = LocalIdx::from_raw(1);
    body.locals = IndexVec::from_raw(vec![
        local_decl(Ty::UNIT, Mutability::Mut),
        local_decl(ty, Mutability::Mut),
    ]);
    let c1 = MirConst {
        kind: MirConstKind::Uint(lhs),
        ty,
        span: Span::DUMMY,
    };
    let c2 = MirConst {
        kind: MirConstKind::Uint(rhs),
        ty,
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(res_local),
                Rvalue::BinaryOp(op, Box::new((Operand::Constant(c1), Operand::Constant(c2)))),
            ),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    body
}

#[test]
fn signed_min_rem_neg1_yields_zero() {
    // i128::MIN % -1 is the only signed remainder that overflows plain `%`.
    // Language semantics require the result to be 0.
    let mut tcx_mut = test_ty_ctx();
    let body = build_binop_body_signed(&mut tcx_mut, BinOp::Rem, i128::MIN, -1);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Int(0))
    );
}

#[test]
fn signed_min_div_neg1_yields_zero() {
    let mut tcx_mut = test_ty_ctx();
    let body = build_binop_body_signed(&mut tcx_mut, BinOp::Div, i128::MIN, -1);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Int(0))
    );
}

#[test]
fn signed_rem_by_zero_is_interp_error() {
    let mut tcx_mut = test_ty_ctx();
    let body = build_binop_body_signed(&mut tcx_mut, BinOp::Rem, 100, 0);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    let res = interp.run_body(&body);
    assert!(matches!(res, Err(InterpError::DivisionByZero)));
}

#[test]
fn signed_div_by_zero_is_interp_error() {
    let mut tcx_mut = test_ty_ctx();
    let body = build_binop_body_signed(&mut tcx_mut, BinOp::Div, 100, 0);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    let res = interp.run_body(&body);
    assert!(matches!(res, Err(InterpError::DivisionByZero)));
}

#[test]
fn signed_rem_normal() {
    let mut tcx_mut = test_ty_ctx();
    let body = build_binop_body_signed(&mut tcx_mut, BinOp::Rem, 17, 5);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Int(2))
    );
}

#[test]
fn unsigned_rem_by_zero_is_interp_error() {
    let mut tcx_mut = test_ty_ctx();
    let body = build_binop_body_uint(&mut tcx_mut, BinOp::Rem, 100u128, 0u128);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    let res = interp.run_body(&body);
    assert!(matches!(res, Err(InterpError::DivisionByZero)));
}

#[test]
fn unsigned_div_by_zero_is_interp_error() {
    let mut tcx_mut = test_ty_ctx();
    let body = build_binop_body_uint(&mut tcx_mut, BinOp::Div, 100u128, 0u128);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    let res = interp.run_body(&body);
    assert!(matches!(res, Err(InterpError::DivisionByZero)));
}

#[test]
fn unsigned_rem_normal() {
    let mut tcx_mut = test_ty_ctx();
    let body = build_binop_body_uint(&mut tcx_mut, BinOp::Rem, 17u128, 5u128);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.run_body(&body).unwrap();
    assert_eq!(
        interp.get_local_value(LocalIdx::from_raw(1)),
        Some(&InterpValue::Uint(2))
    );
}
