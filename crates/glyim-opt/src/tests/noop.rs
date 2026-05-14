use super::super::*;
use glyim_mir::*;
use glyim_test::with_fresh_ty_ctx;
use glyim_core::Mutability;
use std::sync::Arc;

fn make_minimal_body(ctx: &mut glyim_type::TyCtxMut) -> Body {
    let unit_ty = ctx.mk_ty(glyim_type::TyKind::Unit);
    let block = BasicBlockData {
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
    body.basic_blocks = glyim_core::IndexVec::from_raw(vec![block]);
    body.locals = glyim_core::IndexVec::from_raw(vec![LocalDecl {
        ty: unit_ty,
        mutability: Mutability::Not,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    }]);
    body
}

#[test]
fn noop_body_is_unchanged() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| make_minimal_body(ctx_mut));
    let original = body.clone();
    let body = Arc::new(body);
    let optimized = optimize(&ctx, &body);
    assert_eq!(
        optimized.body.basic_blocks.len(),
        original.basic_blocks.len(),
        "block count unchanged"
    );
    assert!(
        matches!(
            optimized.body.basic_blocks[BasicBlockIdx::from_raw(0)].terminator.kind,
            TerminatorKind::Return
        ),
        "terminator unchanged"
    );
}
