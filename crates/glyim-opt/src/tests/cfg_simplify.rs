use glyim_core::Mutability;
use glyim_mir::*;
use glyim_test::with_fresh_ty_ctx;

fn make_body(ctx: &mut glyim_type::TyCtxMut) -> Body {
    let unit_ty = ctx.mk_ty(glyim_type::TyKind::Unit);
    // block0: goto block1
    let block0 = BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Goto {
                target: BasicBlockIdx::from_raw(1),
            },
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        },
        is_cleanup: false,
    };
    // block1: assigns unit to local0, then uses local0 in SwitchInt (so it's live)
    let stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(0)),
            Rvalue::Use(Operand::Constant(MirConst {
                kind: MirConstKind::Unit,
                ty: unit_ty,
                span: glyim_span::Span::DUMMY,
            })),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    let terminator = Terminator {
        kind: TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::new(LocalIdx::from_raw(0))),
            switch_ty: unit_ty,
            targets: SwitchTargets::new(
                vec![(0, BasicBlockIdx::from_raw(2))].into_boxed_slice(),
                BasicBlockIdx::from_raw(2),
            ),
        },
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    let block1 = BasicBlockData {
        statements: vec![stmt],
        terminator,
        is_cleanup: false,
    };
    // block2: return (unreachable but needed as target)
    let block2 = BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        },
        is_cleanup: false,
    };
    let mut body = Body::dummy(glyim_core::DefId::new(
        glyim_core::CrateId::from_raw(0),
        glyim_core::LocalDefId::from_raw(0),
    ));
    body.basic_blocks = glyim_core::IndexVec::from_raw(vec![block0, block1, block2]);
    body.locals = glyim_core::IndexVec::from_raw(vec![LocalDecl {
        ty: unit_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    }]);
    body
}

#[test]
fn merges_single_goto_chain() {
    let (ctx, mut body) = with_fresh_ty_ctx(|ctx_mut| make_body(ctx_mut));
    crate::cfg_simplify::run(&ctx, &mut body);
    // should merge block0 (goto 1) into block1 -> one less block
    assert_eq!(body.basic_blocks.len(), 2, "should have merged block0 into block1, leaving 2 blocks");
    let merged = &body.basic_blocks[BasicBlockIdx::from_raw(0)];
    assert!(!merged.statements.is_empty(), "merged block should contain the statement from block1");
}
