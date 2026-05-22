//! Tests for CFG simplification (merging goto chains)
use glyim_mir::*;
use glyim_type::TyCtx;
use glyim_test::test_frozen_ty_ctx;

#[test]
fn merge_goto_chain() {
    let ctx = test_frozen_ty_ctx();
    let i32_ty = ctx.mk_ty(glyim_type::TyKind::Int(glyim_core::primitives::IntTy::I32));
    let mut body = Body::dummy(Default::default());
    let local0 = LocalIdx::from_raw(0);
    body.locals.push(LocalDecl { ty: i32_ty, mutability: glyim_core::primitives::Mutability::Mut, source_info: SourceInfo::new(glyim_span::Span::DUMMY) });
    // Block A: goto B
    // Block B: assign return, goto C
    // Block C: return
    let bb_a = BasicBlockIdx::from_raw(0);
    let bb_b = BasicBlockIdx::from_raw(1);
    let bb_c = BasicBlockIdx::from_raw(2);
    body.basic_blocks = IndexVec::from_raw(vec![
        BasicBlockData {
            statements: vec![],
            terminator: Terminator { kind: TerminatorKind::Goto { target: bb_b }, source_info: SourceInfo::new(glyim_span::Span::DUMMY) },
            is_cleanup: false,
        },
        BasicBlockData {
            statements: vec![Statement { kind: StatementKind::Assign(Place::new(local0), Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::Int(1), ty: i32_ty, span: glyim_span::Span::DUMMY }))), source_info: SourceInfo::new(glyim_span::Span::DUMMY) }],
            terminator: Terminator { kind: TerminatorKind::Goto { target: bb_c }, source_info: SourceInfo::new(glyim_span::Span::DUMMY) },
            is_cleanup: false,
        },
        BasicBlockData {
            statements: vec![],
            terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(glyim_span::Span::DUMMY) },
            is_cleanup: false,
        },
    ]);
    super::cfg_simplify::run(&ctx, &mut body);
    // After merging, block A should be gone, block B becomes first block with terminator to C
    assert_eq!(body.basic_blocks.len(), 2);
    let first_block = &body.basic_blocks[BasicBlockIdx::from_raw(0)];
    assert!(matches!(first_block.terminator.kind, TerminatorKind::Goto { target } if target == bb_c));
    assert_eq!(first_block.statements.len(), 1);
    let second_block = &body.basic_blocks[BasicBlockIdx::from_raw(1)];
    assert!(matches!(second_block.terminator.kind, TerminatorKind::Return));
}

// TODO: test SwitchInt with single branch converted to Goto
