use glyim_core::Mutability;
use glyim_mir::*;
use glyim_test::with_fresh_ty_ctx;
use glyim_type::TyKind;

fn make_body(ctx: &mut glyim_type::TyCtxMut) -> Body {
    let int_ty = ctx.mk_ty(TyKind::Int(glyim_core::IntTy::I32));
    let const_42 = MirConst {
        kind: MirConstKind::Int(42),
        ty: int_ty,
        span: glyim_span::Span::DUMMY,
    };
    let const_10 = MirConst {
        kind: MirConstKind::Int(10),
        ty: int_ty,
        span: glyim_span::Span::DUMMY,
    };
    let stmt0 = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(1)),
            Rvalue::Use(Operand::Constant(const_42)),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    let stmt1 = Statement {
        kind: StatementKind::Assign(
            Place::new(LocalIdx::from_raw(2)),
            Rvalue::BinaryOp(
                glyim_core::BinOp::Add,
                Box::new((
                    Operand::Copy(Place::new(LocalIdx::from_raw(1))),
                    Operand::Constant(const_10),
                )),
            ),
        ),
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    };
    let block = BasicBlockData {
        statements: vec![stmt0, stmt1],
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
    body.basic_blocks = glyim_core::IndexVec::from_raw(vec![block]);
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
fn propagate_int_constants() {
    let (ctx, mut body) = with_fresh_ty_ctx(|ctx_mut| make_body(ctx_mut));
    crate::constant_prop::run(&ctx, &mut body);
    let block = &body.basic_blocks[BasicBlockIdx::from_raw(0)];
    assert_eq!(block.statements.len(), 2, "should still have two statements");
    if let StatementKind::Assign(_, Rvalue::BinaryOp(_, box_ops)) = &block.statements[1].kind {
        assert!(
            matches!(box_ops.0, Operand::Constant(_)),
            "left operand should be a constant after propagation"
        );
    } else {
        panic!("second statement should be a BinaryOp");
    }
}
