//! Test W4-C02-T02: DCE removes unused `let _ = 42`
use glyim_core::{IndexVec, primitives::IntTy, CrateId, DefId, LocalDefId};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::TyCtx;
use glyim_test::test_ty_ctx;

fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

#[test]
fn dce_removes_unused_assign() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    let local0 = LocalIdx::from_raw(0);
    let local1 = LocalIdx::from_raw(1);
    body.locals.push(LocalDecl { ty: i32_ty, mutability: glyim_core::primitives::Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    body.basic_blocks = IndexVec::from_raw(vec![
        BasicBlockData {
            statements: vec![
                Statement { kind: StatementKind::Assign(Place::new(local1), Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::Int(42), ty: i32_ty, span: Span::DUMMY }))), source_info: SourceInfo::new(Span::DUMMY) },
                Statement { kind: StatementKind::Assign(Place::new(local0), Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::Int(0), ty: i32_ty, span: Span::DUMMY }))), source_info: SourceInfo::new(Span::DUMMY) },
            ],
            terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
            is_cleanup: false,
        }
    ]);
    let ctx = ctx_mut.freeze();
    crate::dce::run(&ctx, &mut body);
    // Only the assignment to local0 (return place) should remain
    assert_eq!(body.basic_blocks[BasicBlockIdx::from_raw(0)].statements.len(), 1);
}
