use glyim_core::Mutability;
use glyim_mir::*;
use glyim_test::with_fresh_ty_ctx;

fn make_body(ctx: &mut glyim_type::TyCtxMut) -> Body {
    let int_ty = ctx.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
    let const_val = MirConst {
        kind: MirConstKind::Int(1),
        ty: int_ty,
        span: glyim_span::Span::DUMMY,
    };
    // dead store to local1
    let dead_stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(1)),
            Rvalue::Use(Operand::Constant(const_val)),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    // live store to local2; local2 is used in the terminator
    let live_stmt = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(2)),
            Rvalue::BinaryOp(
                glyim_core::BinOp::Add,
                Box::new((
                    Operand::Copy(Place::new(LocalIdx::from_raw(0))),
                    Operand::Copy(Place::new(LocalIdx::from_raw(0))),
                )),
            ),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    // terminator uses local2 as discriminant of a SwitchInt (to make it live)
    let terminator = Terminator {
        kind: TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::new(LocalIdx::from_raw(2))),
            switch_ty: int_ty,
            targets: SwitchTargets::new(
                vec![(0, BasicBlockIdx::from_raw(1))].into_boxed_slice(),
                BasicBlockIdx::from_raw(1),
            ),
        },
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    let block0 = BasicBlockData {
        statements: vec![dead_stmt, live_stmt],
        terminator,
        is_cleanup: false,
    };
    // unreachable block1 just to have a target for SwitchInt
    let block1 = BasicBlockData {
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
    body.basic_blocks = glyim_core::IndexVec::from_raw(vec![block0, block1]);
    body.locals = glyim_core::IndexVec::from_raw(vec![
        LocalDecl {
            ty: int_ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        },
        LocalDecl {
            ty: int_ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        },
        LocalDecl {
            ty: int_ty,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        },
    ]);
    body
}

#[test]
fn eliminates_dead_store() {
    let (ctx, mut body) = with_fresh_ty_ctx(|ctx_mut| make_body(ctx_mut));
    crate::dce::run(&ctx, &mut body);
    let block = &body.basic_blocks[BasicBlockIdx::from_raw(0)];
    assert_eq!(block.statements.len(), 1, "dead statement should be removed, live should stay");
    if let StatementKind::Assign(place, _) = &block.statements[0].kind {
        assert_eq!(place.local.to_raw(), 2, "remaining statement should assign to local2");
    } else {
        panic!("expected Assign");
    }
}
