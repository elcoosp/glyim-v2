//! Tests for unreachable block elimination
use glyim_mir::*;
use glyim_type::TyCtx;
use glyim_test::test_frozen_ty_ctx;

#[test]
fn remove_block_after_return() {
    let ctx = test_frozen_ty_ctx();
    let i32_ty = ctx.mk_ty(glyim_type::TyKind::Int(glyim_core::primitives::IntTy::I32));
    let mut body = Body::dummy(Default::default());
    let local0 = LocalIdx::from_raw(0);
    body.locals.push(LocalDecl { ty: i32_ty, mutability: glyim_core::primitives::Mutability::Mut, source_info: SourceInfo::new(glyim_span::Span::DUMMY) });
    // Block0: return
    // Block1: unreachable
    let bb0 = BasicBlockIdx::from_raw(0);
    let bb1 = BasicBlockIdx::from_raw(1);
    body.basic_blocks = IndexVec::from_raw(vec![
        BasicBlockData {
            statements: vec![Statement { kind: StatementKind::Assign(Place::new(local0), Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::Int(42), ty: i32_ty, span: glyim_span::Span::DUMMY }))), source_info: SourceInfo::new(glyim_span::Span::DUMMY) }],
            terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(glyim_span::Span::DUMMY) },
            is_cleanup: false,
        },
        BasicBlockData {
            statements: vec![],
            terminator: Terminator { kind: TerminatorKind::Unreachable, source_info: SourceInfo::new(glyim_span::Span::DUMMY) },
            is_cleanup: false,
        },
    ]);
    super::unreachable_elim::run(&ctx, &mut body);
    assert_eq!(body.basic_blocks.len(), 1);
    assert_eq!(body.basic_blocks[bb0].terminator.kind, TerminatorKind::Return);
}
