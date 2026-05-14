use glyim_core::Mutability;
use glyim_mir::*;
use glyim_test::with_fresh_ty_ctx;

fn make_body(ctx: &mut glyim_type::TyCtxMut) -> Body {
    let unit_ty = ctx.mk_ty(glyim_type::TyKind::Unit);
    let block0 = BasicBlockData {
        statements: vec![],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        },
        is_cleanup: false,
    };
    let block1 = BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(0)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Unit,
                    ty: unit_ty,
                    span: glyim_span::Span::DUMMY,
                })),
            ),
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        }],
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
    body.locals = glyim_core::IndexVec::from_raw(vec![LocalDecl {
        ty: unit_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    }]);
    body
}

#[test]
fn eliminates_unreachable_block() {
    let (ctx, mut body) = with_fresh_ty_ctx(|ctx_mut| make_body(ctx_mut));
    crate::unreachable_elim::run(&ctx, &mut body);
    assert_eq!(body.basic_blocks.len(), 1, "should remove unreachable block");
}
