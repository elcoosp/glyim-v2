use glyim_core::Mutability;
use glyim_mir::*;
use glyim_test::with_fresh_ty_ctx;

fn return_term() -> Terminator {
    Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    }
}
fn goto_term(t: u32) -> Terminator {
    Terminator {
        kind: TerminatorKind::Goto {
            target: BasicBlockIdx::from_raw(t),
        },
        source_info: SourceInfo::new(glyim_span::Span::DUMMY),
    }
}

#[test]
fn eliminates_unreachable_block() {
    let (ctx, mut body) = with_fresh_ty_ctx(|ctx_mut| {
        let ut = ctx_mut.mk_ty(glyim_type::TyKind::Unit);
        let block0 = BasicBlockData {
            statements: vec![],
            terminator: return_term(),
            is_cleanup: false,
        };
        let block1 = BasicBlockData {
            statements: vec![],
            terminator: return_term(),
            is_cleanup: false,
        };
        let mut body = Body::dummy(glyim_core::DefId::new(
            glyim_core::CrateId::from_raw(0),
            glyim_core::LocalDefId::from_raw(0),
        ));
        body.basic_blocks = glyim_core::IndexVec::from_raw(vec![block0, block1]);
        body.locals = glyim_core::IndexVec::from_raw(vec![LocalDecl {
            ty: ut,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        }]);
        body
    });
    crate::unreachable_elim::run(&ctx, &mut body);
    assert_eq!(
        body.basic_blocks.len(),
        1,
        "should remove unreachable block"
    );
}

#[test]
fn all_reachable_no_change() {
    let (ctx, mut body) = with_fresh_ty_ctx(|ctx_mut| {
        let ut = ctx_mut.mk_ty(glyim_type::TyKind::Unit);
        let block0 = BasicBlockData {
            statements: vec![],
            terminator: goto_term(1),
            is_cleanup: false,
        };
        let block1 = BasicBlockData {
            statements: vec![],
            terminator: return_term(),
            is_cleanup: false,
        };
        let mut body = Body::dummy(glyim_core::DefId::new(
            glyim_core::CrateId::from_raw(0),
            glyim_core::LocalDefId::from_raw(0),
        ));
        body.basic_blocks = glyim_core::IndexVec::from_raw(vec![block0, block1]);
        body.locals = glyim_core::IndexVec::from_raw(vec![LocalDecl {
            ty: ut,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        }]);
        body
    });
    crate::unreachable_elim::run(&ctx, &mut body);
    assert_eq!(
        body.basic_blocks.len(),
        2,
        "all reachable, no change expected"
    );
}

#[test]
fn multiple_unreachable_removed() {
    let (ctx, mut body) = with_fresh_ty_ctx(|ctx_mut| {
        let ut = ctx_mut.mk_ty(glyim_type::TyKind::Unit);
        // block0: return; block1: unreachable; block2: unreachable
        let block0 = BasicBlockData {
            statements: vec![],
            terminator: return_term(),
            is_cleanup: false,
        };
        let block1 = BasicBlockData {
            statements: vec![],
            terminator: return_term(),
            is_cleanup: false,
        };
        let block2 = BasicBlockData {
            statements: vec![],
            terminator: return_term(),
            is_cleanup: false,
        };
        let mut body = Body::dummy(glyim_core::DefId::new(
            glyim_core::CrateId::from_raw(0),
            glyim_core::LocalDefId::from_raw(0),
        ));
        body.basic_blocks = glyim_core::IndexVec::from_raw(vec![block0, block1, block2]);
        body.locals = glyim_core::IndexVec::from_raw(vec![LocalDecl {
            ty: ut,
            mutability: Mutability::Not,
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        }]);
        body
    });
    crate::unreachable_elim::run(&ctx, &mut body);
    assert_eq!(
        body.basic_blocks.len(),
        1,
        "should remove both unreachable blocks"
    );
}
