use crate::*;
use glyim_core::{CrateId, DefId, LocalDefId, Mutability, IntTy, FloatTy, UintTy};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{Ty, TyKind, Const, ConstKind, TyCtxMut, GenericArg};

pub fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

pub fn local_decl(ty: Ty, mutability: Mutability) -> LocalDecl {
    LocalDecl { ty, mutability, source_info: SourceInfo::new(Span::DUMMY) }
}

pub fn empty_body(ret_ty: Ty) -> Body {
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![local_decl(ret_ty, Mutability::Mut)]);
    body
}

pub fn add_local(body: &mut Body, ty: Ty, mutability: Mutability) -> LocalIdx {
    let idx = LocalIdx::from_raw(body.locals.len() as u32);
    body.locals.push(local_decl(ty, mutability));
    idx
}

pub fn add_statement(body: &mut Body, bb: BasicBlockIdx, stmt: StatementKind) {
    while body.basic_blocks.len() <= bb.index() {
        body.basic_blocks.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator { kind: TerminatorKind::Unreachable, source_info: SourceInfo::new(Span::DUMMY) },
            is_cleanup: false,
        });
    }
    body.basic_blocks[bb].statements.push(Statement { kind: stmt, source_info: SourceInfo::new(Span::DUMMY) });
}

pub fn set_terminator(body: &mut Body, bb: BasicBlockIdx, kind: TerminatorKind) {
    while body.basic_blocks.len() <= bb.index() {
        body.basic_blocks.push(BasicBlockData {
            statements: vec![],
            terminator: Terminator { kind: TerminatorKind::Unreachable, source_info: SourceInfo::new(Span::DUMMY) },
            is_cleanup: false,
        });
    }
    body.basic_blocks[bb].terminator.kind = kind;
}

pub fn const_int(val: i128) -> Operand {
    Operand::Constant(MirConst {
        kind: MirConstKind::Int(val),
        ty: Ty::INT,
        span: Span::DUMMY,
    })
}

pub fn const_bool(val: bool) -> Operand {
    Operand::Constant(MirConst {
        kind: MirConstKind::Bool(val),
        ty: Ty::BOOL,
        span: Span::DUMMY,
    })
}

pub fn mir_const_usize(tcx: &mut TyCtxMut, val: u64) -> MirConst {
    MirConst {
        kind: MirConstKind::Uint(val),
        ty: tcx.mk_ty(TyKind::Uint(UintTy::Usize)),
        span: Span::DUMMY,
    }
}

pub fn mk_array_ty(tcx: &mut TyCtxMut, elem_ty: Ty, len: u64) -> Ty {
    let usize_ty = tcx.mk_ty(TyKind::Uint(UintTy::Usize));
    let const_len = Const {
        kind: ConstKind::Uint(len),
        ty: usize_ty,
    };
    tcx.mk_ty(TyKind::Array(elem_ty, const_len))
}

pub fn place_with_proj(local: LocalIdx, proj: &[ProjectionElem]) -> Place {
    Place {
        local,
        projection: proj.to_vec().into_boxed_slice(),
    }
}

pub fn build_allocation_body(tcx: &mut TyCtxMut, val: i128) -> Body {
    let i32_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = empty_body(Ty::UNIT);
    let local = add_local(&mut body, i32_ty, Mutability::Mut);
    let bb0 = BasicBlockIdx::from_raw(0);
    add_statement(&mut body, bb0, StatementKind::StorageLive(local));
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local), Rvalue::Use(const_int(val))));
    set_terminator(&mut body, bb0, TerminatorKind::Return);
    body
}

pub fn build_add_body(tcx: &TyCtxMut, a: i128, b: i128, ty: Ty) -> Body {
    let mut body = empty_body(Ty::UNIT);
    let local_a = add_local(&mut body, ty, Mutability::Mut);
    let local_b = add_local(&mut body, ty, Mutability::Mut);
    let local_res = add_local(&mut body, ty, Mutability::Mut);
    let bb0 = BasicBlockIdx::from_raw(0);
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local_a), Rvalue::Use(const_int(a))));
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local_b), Rvalue::Use(const_int(b))));
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local_res), Rvalue::BinaryOp(BinOp::Add, Box::new((Operand::Copy(Place::new(local_a)), Operand::Copy(Place::new(local_b)))))));
    set_terminator(&mut body, bb0, TerminatorKind::Return);
    body
}

pub fn build_sub_body(tcx: &TyCtxMut, a: i128, b: i128, ty: Ty) -> Body {
    let mut body = empty_body(Ty::UNIT);
    let local_a = add_local(&mut body, ty, Mutability::Mut);
    let local_b = add_local(&mut body, ty, Mutability::Mut);
    let local_res = add_local(&mut body, ty, Mutability::Mut);
    let bb0 = BasicBlockIdx::from_raw(0);
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local_a), Rvalue::Use(const_int(a))));
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local_b), Rvalue::Use(const_int(b))));
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local_res), Rvalue::BinaryOp(BinOp::Sub, Box::new((Operand::Copy(Place::new(local_a)), Operand::Copy(Place::new(local_b)))))));
    set_terminator(&mut body, bb0, TerminatorKind::Return);
    body
}

pub fn build_branch_body(tcx: &mut TyCtxMut, cond_true: bool, then_bb: BasicBlockIdx, else_bb: BasicBlockIdx) -> Body {
    let bool_ty = tcx.mk_ty(TyKind::Bool);
    let mut body = empty_body(Ty::UNIT);
    let local_cond = add_local(&mut body, bool_ty, Mutability::Mut);
    let bb0 = BasicBlockIdx::from_raw(0);
    add_statement(&mut body, bb0, StatementKind::Assign(Place::new(local_cond), Rvalue::Use(const_bool(cond_true))));
    set_terminator(&mut body, bb0, TerminatorKind::SwitchInt {
        discr: Operand::Copy(Place::new(local_cond)),
        switch_ty: bool_ty,
        targets: SwitchTargets::if_switch(then_bb, else_bb),
    });
    body
}

pub fn build_infinite_loop_body() -> Body {
    let mut body = empty_body(Ty::UNIT);
    let bb0 = BasicBlockIdx::from_raw(0);
    set_terminator(&mut body, bb0, TerminatorKind::Goto { target: bb0 });
    body
}

pub fn build_recursive_body(def_id: DefId) -> Body {
    let mut body = empty_body(Ty::UNIT);
    let bb0 = BasicBlockIdx::from_raw(0);
    let dummy_local = add_local(&mut body, Ty::UNIT, Mutability::Mut);
    let call = TerminatorKind::Call {
        func: Operand::Constant(MirConst {
            kind: MirConstKind::Fn(glyim_core::def_id::FnDefId::from_raw(def_id.to_raw()), glyim_type::Substitution::empty()),
            ty: Ty::UNIT,
            span: Span::DUMMY,
        }),
        args: vec![],
        destination: Place::new(dummy_local),
        target: Some(bb0),
        cleanup: None,
    };
    set_terminator(&mut body, bb0, call);
    body
}

pub fn build_unreachable_body() -> Body {
    let mut body = empty_body(Ty::UNIT);
    let bb0 = BasicBlockIdx::from_raw(0);
    set_terminator(&mut body, bb0, TerminatorKind::Unreachable);
    body
}
