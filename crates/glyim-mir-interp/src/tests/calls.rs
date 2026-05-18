use crate::*;
use glyim_core::{CrateId, DefId, IndexVec, IntTy, LocalDefId, Mutability};
use glyim_span::Span;
use glyim_test::test_ty_ctx;
use glyim_type::{Ty, TyCtxMut, TyKind};

fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

fn build_callee_body(tcx: &mut TyCtxMut, val: i128) -> Body {
    let mut body = Body::dummy(dummy_def_id());
    let ret_local = LocalIdx::from_raw(0);
    let i32_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    body.locals = IndexVec::from_raw(vec![LocalDecl {
        ty: i32_ty,
        mutability: Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    }]);
    let c = MirConst {
        kind: MirConstKind::Int(val),
        ty: i32_ty,
        span: Span::DUMMY,
    };
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(Place::new(ret_local), Rvalue::Use(Operand::Constant(c))),
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
fn interpret_function_call() {
    let mut tcx_mut = test_ty_ctx();
    let callee_id = dummy_def_id();
    let callee_body = build_callee_body(&mut tcx_mut, 42);

    let i32_ty = tcx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let caller_body = {
        let mut body = Body::dummy(dummy_def_id());
        let ret_local = LocalIdx::from_raw(1);
        body.locals = IndexVec::from_raw(vec![
            LocalDecl {
                ty: Ty::UNIT,
                mutability: Mutability::Mut,
                source_info: SourceInfo::new(Span::DUMMY),
            },
            LocalDecl {
                ty: i32_ty,
                mutability: Mutability::Mut,
                source_info: SourceInfo::new(Span::DUMMY),
            },
        ]);
        body.basic_blocks = IndexVec::from_raw(vec![
            BasicBlockData {
                statements: vec![],
                terminator: Terminator {
                    kind: TerminatorKind::Call {
                        func: Operand::Constant(MirConst {
                            kind: MirConstKind::Int(0), // placeholder; interpreter resolves by def_id
                            ty: i32_ty,
                            span: Span::DUMMY,
                        }),
                        args: vec![],
                        destination: Place::new(ret_local),
                        target: Some(BasicBlockIdx::from_raw(1)),
                        cleanup: None,
                    },
                    source_info: SourceInfo::new(Span::DUMMY),
                },
                is_cleanup: false,
            },
            BasicBlockData {
                statements: vec![],
                terminator: Terminator {
                    kind: TerminatorKind::Return,
                    source_info: SourceInfo::new(Span::DUMMY),
                },
                is_cleanup: false,
            },
        ]);
        body
    };
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.add_function(callee_id, callee_body);
    interp.run_body(&caller_body).unwrap();
    let val = interp.get_local_value(LocalIdx::from_raw(1)).unwrap();
    assert_eq!(val, &InterpValue::Int(42));
}
