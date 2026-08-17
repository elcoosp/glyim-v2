use crate::*;
use glyim_core::{CrateId, DefId, IndexVec, IntTy, LocalDefId, Mutability};
use glyim_mir::{AssertMessage, BasicBlockData, BasicBlockIdx, LocalDecl, LocalIdx, MirConst, MirConstKind, Operand, Place, Rvalue, SourceInfo, Statement, StatementKind, Terminator, TerminatorKind};
use glyim_span::Span;
use glyim_test::test_ty_ctx;
use glyim_type::{Ty, TyCtxMut, TyKind};

fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

/// Build a body whose single "real" block panics via a failing `Assert` that
/// carries a `cleanup` edge to a cleanup block. The cleanup block records that
/// it ran (by writing a sentinel into a tracking local) and then Gotos the
/// normal return block. Local layout:
///   _0: UNIT (return slot)
///   _1: i32  (tracking local: 0 = not run, 42 = cleanup ran)
fn build_unwinding_body(tcx: &mut TyCtxMut) -> Body {
    let i32_ty = tcx.mk_ty(TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
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
        // BB0: the main block — a failing assert with a cleanup edge.
        BasicBlockData {
            statements: vec![],
            terminator: Terminator {
                kind: TerminatorKind::Assert {
                    cond: Operand::Constant(MirConst {
                        kind: MirConstKind::Bool(false),
                        ty: tcx.bool_ty(),
                        span: Span::DUMMY,
                    }),
                    expected: true,
                    target: BasicBlockIdx::from_raw(2),
                    cleanup: Some(BasicBlockIdx::from_raw(1)),
                    msg: AssertMessage::BoundsCheck,
                },
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: false,
        },
        // BB1: cleanup block — record that cleanup ran, then Goto return.
        BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::new(LocalIdx::from_raw(1)),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(42),
                        ty: i32_ty,
                        span: Span::DUMMY,
                    })),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            }],
            terminator: Terminator {
                kind: TerminatorKind::Goto {
                    target: BasicBlockIdx::from_raw(2),
                },
                source_info: SourceInfo::new(Span::DUMMY),
            },
            is_cleanup: true,
        },
        // BB2: normal return (reached only via cleanup when unwinding).
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
}

#[test]
fn panic_without_unwind_aborts() {
    // Plan §14.2: with `panics_unwind` off (the default), a failing assert
    // still aborts interpretation with an error — the old behavior is preserved.
    let mut tcx_mut = test_ty_ctx();
    let body = build_unwinding_body(&mut tcx_mut);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx);
    let result = interp.run_body(&body);
    assert!(result.is_err(), "without unwinding, a panic must abort");
}

#[test]
fn panic_runs_cleanup_block_when_unwinding() {
    // Plan §14.2: with `panics_unwind` on, a failing assert routes to the
    // `cleanup` edge; the cleanup block runs and records that it executed,
    // then the function reaches a clean Return.
    let mut tcx_mut = test_ty_ctx();
    let body = build_unwinding_body(&mut tcx_mut);
    let tcx = tcx_mut.freeze();
    let mut interp = Interpreter::new(&tcx).with_panics_unwind(true);
    interp.run_body(&body).expect("with unwinding, the cleanup block must absorb the panic");
    let ran = interp.get_local_value(LocalIdx::from_raw(1)).expect("tracking local must be set");
    assert_eq!(ran, &InterpValue::Int(42), "cleanup block must have run during unwind");
}
