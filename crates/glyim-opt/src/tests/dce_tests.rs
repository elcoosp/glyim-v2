//! Tests for dead code elimination
use glyim_mir::*;
use glyim_type::TyCtx;
use glyim_test::{test_frozen_ty_ctx, with_fresh_ty_ctx};

#[test]
fn dce_removes_unused_assign() {
    with_fresh_ty_ctx(|ctx_mut| {
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::primitives::IntTy::I32));
        let mut body = Body::dummy(Default::default());
        let local0 = LocalIdx::from_raw(0);
        let local1 = LocalIdx::from_raw(1); // unused
        body.locals.push(LocalDecl { ty: i32_ty, mutability: glyim_core::primitives::Mutability::Mut, source_info: SourceInfo::new(glyim_span::Span::DUMMY) });
        body.basic_blocks = IndexVec::from_raw(vec![
            BasicBlockData {
                statements: vec![
                    Statement { kind: StatementKind::Assign(Place::new(local1), Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::Int(42), ty: i32_ty, span: glyim_span::Span::DUMMY }))), source_info: SourceInfo::new(glyim_span::Span::DUMMY) },
                ],
                terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(glyim_span::Span::DUMMY) },
                is_cleanup: false,
            }
        ]);
        let ctx = ctx_mut.freeze();
        super::dce::run(&ctx, &mut body);
        assert_eq!(body.basic_blocks[BasicBlockIdx::from_raw(0)].statements.len(), 0);
    });
}
