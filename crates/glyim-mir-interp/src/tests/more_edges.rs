use crate::Interpreter;
use glyim_core::{CrateId, DefId, LocalDefId, IntTy, Mutability, IndexVec};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{Ty, TyKind, TyCtxMut};
use glyim_test::test_ty_ctx;

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

#[test]
fn fn_ptr_to_ptr_cast_returns_success() {
    let mut tcx = test_ty_ctx();
    let i32_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let ptr_ty = tcx.mk_ty(TyKind::RawPtr(i32_ty, Mutability::Not));
    let const_val = MirConst {
        kind: MirConstKind::Int(42),
        ty: i32_ty,
        span: Span::DUMMY,
    };
    let operand = Operand::Constant(const_val);
    let cast_rvalue = Rvalue::Cast(CastKind::PtrToPtr, operand, ptr_ty);
    let assign_stmt = Statement {
        kind: StatementKind::Assign(Place::new(LocalIdx::from_raw(0)), cast_rvalue),
        source_info: SourceInfo::new(Span::DUMMY),
    };
    let mut body = empty_body(ptr_ty);
    body.locals = IndexVec::from_raw(vec![local_decl(ptr_ty, Mutability::Mut)]);
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![assign_stmt],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let tcx_frozen = tcx.freeze();
    let mut interp = Interpreter::new(&tcx_frozen);
    interp.add_function(dummy_def_id(), body.clone());
    let res = interp.run_body(&body);
    assert!(res.is_ok());
}
