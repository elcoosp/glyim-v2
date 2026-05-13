use super::common::*;
use crate::*;
use glyim_core::{CrateId, DefId, LocalDefId, IndexVec, Mutability};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{IntTy, Ty, TyCtx, TyKind};

fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

fn build_callee_body(tcx: &TyCtx, val: i128) -> Body {
    let mut body = Body::dummy(dummy_def_id());
    let ret_local = LocalIdx::from_raw(1);
    let i32_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    body.locals = IndexVec::from_raw(vec![
        LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) },
        LocalDecl { ty: i32_ty.clone(), mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) },
    ]);
    let c = MirConst { kind: MirConstKind::Int(val), ty: i32_ty, span: Span::DUMMY };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(Place::new(ret_local), Rvalue::Use(Operand::Constant(c))),
            source_info: SourceInfo::new(Span::DUMMY),
        }],
        terminator: Some(Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        }),
        is_cleanup: false,
    }]);
    body
}

#[test]
fn interpret_function_call() {
    let tcx = glyim_test::test_ty_ctx().freeze();
    let callee_id = dummy_def_id();
    let callee_body = build_callee_body(&tcx, 42);
    let caller_body = {
        let mut body = Body::dummy(dummy_def_id());
        let ret_local = LocalIdx::from_raw(1);
        let i32_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
        body.locals = IndexVec::from_raw(vec![
            LocalDecl { ty: Ty::UNIT, mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) },
            LocalDecl { ty: i32_ty.clone(), mutability: Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) },
        ]);
        body.basic_blocks = IndexVec::from_raw(vec![
            BasicBlockData {
                statements: vec![],
                terminator: Some(Terminator {
                    kind: TerminatorKind::Call {
                        func: Operand::Constant(MirConst {
                            kind: MirConstKind::FnDef(callee_id),
                            ty: i32_ty,
                            span: Span::DUMMY,
                        }),
                        args: vec![],
                        destination: (Place::new(ret_local), BasicBlockIdx::from_raw(1)),
                        target: Some(BasicBlockIdx::from_raw(1)),
                        cleanup: None,
                    },
                    source_info: SourceInfo::new(Span::DUMMY),
                }),
                is_cleanup: false,
            },
            BasicBlockData {
                statements: vec![],
                terminator: Some(Terminator {
                    kind: TerminatorKind::Return,
                    source_info: SourceInfo::new(Span::DUMMY),
                }),
                is_cleanup: false,
            },
        ]);
        body
    };
    let mut interp = Interpreter::new(tcx.clone());
    interp.add_function(callee_id, callee_body);
    interp.run_body(&caller_body).unwrap();
    let val = interp.get_local_value(LocalIdx::from_raw(1)).unwrap();
    assert_eq!(val, &InterpValue::Int(42));
}
