use super::super::*;
use glyim_core::Mutability;
use glyim_mir::*;
use glyim_test::with_fresh_ty_ctx;
use std::sync::Arc;

fn make_place(idx: u32) -> Place {
    Place::new(LocalIdx::from_raw(idx))
}
fn make_copy(idx: u32) -> Operand {
    Operand::Copy(make_place(idx))
}

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

fn make_live_multi_block(ctx: &mut glyim_type::TyCtxMut) -> Body {
    let int_ty = ctx.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
    // block0: local1 = 1; goto block1
    let block0 = BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                make_place(1),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Int(1),
                    ty: int_ty,
                    span: glyim_span::Span::DUMMY,
                })),
            ),
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Goto {
                target: BasicBlockIdx::from_raw(1),
            },
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        },
        is_cleanup: false,
    };
    // block1: local2 = local1 + local1 ; switch(local2) -> return
    let block1 = BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                make_place(2),
                Rvalue::BinaryOp(
                    glyim_core::BinOp::Add,
                    Box::new((make_copy(1), make_copy(1))),
                ),
            ),
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::SwitchInt {
                discr: make_copy(2),
                switch_ty: int_ty,
                targets: SwitchTargets::new(
                    vec![(0, BasicBlockIdx::from_raw(2))].into_boxed_slice(),
                    BasicBlockIdx::from_raw(2),
                ),
            },
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        },
        is_cleanup: false,
    };
    // block2: return
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
    body.locals = glyim_core::IndexVec::from_raw(
        (0..3)
            .map(|_| LocalDecl {
                ty: int_ty,
                mutability: Mutability::Not,
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            })
            .collect(),
    );
    body
}

/// Build a body where no pass can eliminate the assignment: use a non-constant
/// operand (local0) so constant-prop cannot replace it, keeping local1 live.
fn make_body_optimization_resistant(ctx: &mut glyim_type::TyCtxMut) -> Body {
    let int_ty = ctx.mk_ty(glyim_type::TyKind::Int(glyim_core::IntTy::I32));
    // block0: local1 = local0 (copy of param, not a constant); goto block1
    let block0 = BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(make_place(1), Rvalue::Use(make_copy(0))),
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::Goto {
                target: BasicBlockIdx::from_raw(1),
            },
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        },
        is_cleanup: false,
    };
    // block1: local2 = local1 + local1 ; SwitchInt(local2) -> return
    let block1 = BasicBlockData {
        statements: vec![Statement {
            kind: StatementKind::Assign(
                make_place(2),
                Rvalue::BinaryOp(
                    glyim_core::BinOp::Add,
                    Box::new((make_copy(1), make_copy(1))),
                ),
            ),
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        }],
        terminator: Terminator {
            kind: TerminatorKind::SwitchInt {
                discr: make_copy(2),
                switch_ty: int_ty,
                targets: SwitchTargets::new(
                    vec![(0, BasicBlockIdx::from_raw(2))].into_boxed_slice(),
                    BasicBlockIdx::from_raw(2),
                ),
            },
            source_info: SourceInfo::new(glyim_span::Span::DUMMY),
        },
        is_cleanup: false,
    };
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
    body.locals = glyim_core::IndexVec::from_raw(
        (0..3)
            .map(|_| LocalDecl {
                ty: int_ty,
                mutability: Mutability::Not,
                source_info: SourceInfo::new(glyim_span::Span::DUMMY),
            })
            .collect(),
    );
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
            optimized.body.basic_blocks[BasicBlockIdx::from_raw(0)]
                .terminator
                .kind,
            TerminatorKind::Return
        ),
        "terminator unchanged"
    );
}

#[test]
fn idempotent_optimization() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| make_body_optimization_resistant(ctx_mut));
    let body = Arc::new(body);
    let opt1 = optimize(&ctx, &body);
    let body2 = Arc::new(opt1.body.clone());
    let opt2 = optimize(&ctx, &body2);
    assert_eq!(
        opt1.body.basic_blocks.len(),
        opt2.body.basic_blocks.len(),
        "idempotent: same block count"
    );
    for (bb1, bb2) in opt1
        .body
        .basic_blocks
        .iter()
        .zip(opt2.body.basic_blocks.iter())
    {
        assert_eq!(
            bb1.statements.len(),
            bb2.statements.len(),
            "idempotent: same statement count per block"
        );
    }
}

#[test]
fn non_empty_body_preserves_semantics() {
    let (ctx, body) = with_fresh_ty_ctx(|ctx_mut| make_body_optimization_resistant(ctx_mut));
    let body = Arc::new(body);
    let optimized = optimize(&ctx, &body);
    let total_stmts: usize = optimized
        .body
        .basic_blocks
        .iter()
        .map(|b| b.statements.len())
        .sum();
    let orig_total: usize = body.basic_blocks.iter().map(|b| b.statements.len()).sum();
    assert_eq!(
        total_stmts, orig_total,
        "all live statements preserved after optimization when no constant folding applies"
    );
}
