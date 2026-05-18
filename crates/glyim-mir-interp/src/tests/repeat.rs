use crate::*;
use glyim_core::{CrateId, DefId, LocalDefId, IntTy, UintTy, Mutability, IndexVec};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{Ty, TyKind, Const, ConstKind, TyCtxMut};

fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

fn local_decl(ty: Ty, mutability: Mutability) -> LocalDecl {
    LocalDecl { ty, mutability, source_info: SourceInfo::new(Span::DUMMY) }
}

fn mk_array_ty(tcx: &mut TyCtxMut, elem_ty: Ty, len: u64) -> Ty {
    let usize_ty = tcx.mk_ty(TyKind::Uint(UintTy::Usize));
    let const_len = Const {
        kind: ConstKind::Uint(len.into()),
        ty: usize_ty,
    };
    tcx.mk_ty(TyKind::Array(elem_ty, const_len))
}

#[test]
fn test_t09_repeat_rvalue() {
    let mut tcx = glyim_test::test_ty_ctx();
    let elem_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let array_ty = mk_array_ty(&mut tcx, elem_ty, 3);
    let const_val = MirConst {
        kind: MirConstKind::Int(5),
        ty: elem_ty,
        span: Span::DUMMY,
    };
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
    let mut body = Body::dummy(dummy_def_id());
    body.locals = IndexVec::from_raw(vec![local_decl(array_ty, Mutability::Mut)]);
    let bb_data = BasicBlockData {
        statements: vec![assign_stmt],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    };
    body.basic_blocks.push(bb_data);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.add_function(dummy_def_id(), body);
    interp.run_body(&body).unwrap();
    let ret_val = interp.get_local_value(LocalIdx::from_raw(0)).cloned().unwrap();
    let expected = InterpValue::Aggregate(vec![InterpValue::Int(5); 3]);
    assert_eq!(ret_val, expected);
}
