//! Test W4-C02-T03: CFG simplify merges `goto` chains
//! After merging, the entire chain (including the return) collapses into a single block.
use glyim_core::{IndexVec, primitives::IntTy, CrateId, DefId, LocalDefId};
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::TyCtx;
use glyim_test::test_ty_ctx;

fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

#[test]
fn merge_goto_chain() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    let local0 = LocalIdx::from_raw(0);
    body.locals.push(LocalDecl { ty: i32_ty, mutability: glyim_core::primitives::Mutability::Mut, source_info: SourceInfo::new(Span::DUMMY) });
    let bb_b = BasicBlockIdx::from_raw(1);
    let bb_c = BasicBlockIdx::from_raw(2);
    body.basic_blocks = IndexVec::from_raw(vec![
        BasicBlockData {
            statements: vec![],
            terminator: Terminator { kind: TerminatorKind::Goto { target: bb_b }, source_info: SourceInfo::new(Span::DUMMY) },
            is_cleanup: false,
        },
        BasicBlockData {
            statements: vec![Statement { kind: StatementKind::Assign(Place::new(local0), Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::Int(1), ty: i32_ty, span: Span::DUMMY }))), source_info: SourceInfo::new(Span::DUMMY) }],
            terminator: Terminator { kind: TerminatorKind::Goto { target: bb_c }, source_info: SourceInfo::new(Span::DUMMY) },
            is_cleanup: false,
        },
        BasicBlockData {
            statements: vec![],
            terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(Span::DUMMY) },
            is_cleanup: false,
        },
    ]);
    let ctx = ctx_mut.freeze();
    crate::cfg_simplify::run(&ctx, &mut body);
    // All three blocks should be merged into one block with a Return terminator.
    assert_eq!(body.basic_blocks.len(), 1);
    let block = &body.basic_blocks[BasicBlockIdx::from_raw(0)];
    assert!(matches!(block.terminator.kind, TerminatorKind::Return));
    // The assign statement from block B should be present.
    assert_eq!(block.statements.len(), 1);
}
