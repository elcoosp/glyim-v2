use crate::*;
use glyim_core::{CrateId, DefId, LocalDefId, IntTy, FloatTy, UintTy, Mutability, IndexVec};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{Ty, TyKind, Const, ConstKind, TyCtxMut};

fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

fn local_decl(ty: Ty, mutability: Mutability) -> LocalDecl {
    LocalDecl { ty, mutability, source_info: SourceInfo::new(Span::DUMMY) }
}

fn empty_body(ret_ty: Ty) -> Body {
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![local_decl(ret_ty, Mutability::Mut)]);
    body
}

fn add_local(body: &mut Body, ty: Ty, mutability: Mutability) -> LocalIdx {
    let idx = LocalIdx::from_raw(body.locals.len() as u32);
    body.locals.push(local_decl(ty, mutability));
    idx
}

fn add_statement(body: &mut Body, bb: BasicBlockIdx, stmt: StatementKind) {
    while body.basic_blocks.len() <= bb.index() {
        body.basic_blocks.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator { kind: TerminatorKind::Unreachable, source_info: SourceInfo::new(Span::DUMMY) },
            is_cleanup: false,
        });
    }
    body.basic_blocks[bb].statements.push(Statement { kind: stmt, source_info: SourceInfo::new(Span::DUMMY) });
}

fn set_terminator(body: &mut Body, bb: BasicBlockIdx, kind: TerminatorKind) {
    while body.basic_blocks.len() <= bb.index() {
        body.basic_blocks.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator { kind: TerminatorKind::Unreachable, source_info: SourceInfo::new(Span::DUMMY) },
            is_cleanup: false,
        });
    }
    body.basic_blocks[bb].terminator.kind = kind;
}

fn const_int(_tcx: &mut TyCtxMut, val: i128, ty: Ty) -> Operand {
    Operand::Constant(MirConst { kind: MirConstKind::Int(val), ty, span: Span::DUMMY })
}

fn mk_array_ty(tcx: &mut TyCtxMut, elem_ty: Ty, len: u64) -> Ty {
    let usize_ty = tcx.mk_ty(TyKind::Uint(UintTy::Usize));
    let const_len = Const { kind: ConstKind::Uint(len.into()), ty: usize_ty };
    tcx.mk_ty(TyKind::Array(elem_ty, const_len))
}

#[test]
fn cast_ptr_to_ptr_returns_value() {
    let mut tcx = test_ty_ctx();
    let i32_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let ptr_ty = tcx.mk_ty(TyKind::RawPtr(i32_ty, Mutability::Not));
    let const_val = MirConst { kind: MirConstKind::Int(42), ty: i32_ty, span: Span::DUMMY };
    let cast_rvalue = Rvalue::Cast(CastKind::PtrToPtr, Operand::Constant(const_val), ptr_ty);
    let assign_stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(0)), cast_rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let mut body = empty_body(ptr_ty);
    body.locals = IndexVec::from_raw(vec![local_decl(ptr_ty, Mutability::Mut)]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![assign_stmt],
        terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
        is_cleanup: false,
    }]);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.add_function(dummy_def_id(), body);
    let res = interp.run_body(&interp.function_table.values().next().unwrap().clone());
    assert!(res.is_ok());
    let ret_val = interp.get_local_value(LocalIdx::from_raw(0)).cloned().unwrap();
    assert_eq!(ret_val, InterpValue::Int(42));
}

#[test]
fn float_const_returns_value() {
    let mut tcx = test_ty_ctx();
    let float_ty = tcx.mk_ty(TyKind::Float(FloatTy::F64));
    let const_val = MirConst {
        kind: MirConstKind::FloatBits(42.0_f64.to_bits()),
        ty: float_ty,
        span: Span::DUMMY,
    };
    let assign_stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(0)), Rvalue::Use(Operand::Constant(const_val))),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let mut body = empty_body(float_ty);
    body.locals = IndexVec::from_raw(vec![local_decl(float_ty, Mutability::Mut)]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![assign_stmt],
        terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
        is_cleanup: false,
    }]);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.add_function(dummy_def_id(), body);
    let res = interp.run_body(&interp.function_table.values().next().unwrap().clone());
    assert!(res.is_ok());
    let ret_val = interp.get_local_value(LocalIdx::from_raw(0)).cloned().unwrap();
    assert_eq!(ret_val, InterpValue::Float(42.0));
}

#[test]
fn repeat_rvalue_returns_array() {
    let mut tcx = test_ty_ctx();
    let elem_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let array_ty = mk_array_ty(&mut tcx, elem_ty, 3);
    let const_val = MirConst { kind: MirConstKind::Int(7), ty: elem_ty, span: Span::DUMMY };
    let count_const = MirConst {
        kind: MirConstKind::Uint(3),
        ty: tcx.mk_ty(TyKind::Uint(UintTy::Usize)),
        span: Span::DUMMY,
    };
    let repeat_rvalue = Rvalue::Repeat(Operand::Constant(const_val), count_const);
    let assign_stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(0)), repeat_rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let mut body = empty_body(array_ty);
    body.locals = IndexVec::from_raw(vec![local_decl(array_ty, Mutability::Mut)]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![assign_stmt],
        terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
        is_cleanup: false,
    }]);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.add_function(dummy_def_id(), body);
    interp.run_body(&body).unwrap();
    let ret_val = interp.get_local_value(LocalIdx::from_raw(0)).cloned().unwrap();
    let expected = InterpValue::Aggregate(vec![InterpValue::Int(7); 3]);
    assert_eq!(ret_val, expected);
}

#[test]
fn string_const_returns_value() {
    let mut tcx = test_ty_ctx();
    let name = tcx.resolver().intern("hello");
    let string_ty = tcx.mk_ty(TyKind::String);
    let const_val = MirConst { kind: MirConstKind::String(name), ty: string_ty, span: Span::DUMMY };
    let assign_stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(0)), Rvalue::Use(Operand::Constant(const_val))),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let mut body = empty_body(string_ty);
    body.locals = IndexVec::from_raw(vec![local_decl(string_ty, Mutability::Mut)]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![assign_stmt],
        terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
        is_cleanup: false,
    }]);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.add_function(dummy_def_id(), body);
    let res = interp.run_body(&interp.function_table.values().next().unwrap().clone());
    assert!(res.is_ok());
    let ret_val = interp.get_local_value(LocalIdx::from_raw(0)).cloned().unwrap();
    assert_eq!(ret_val, InterpValue::String("hello".to_string()));
}
