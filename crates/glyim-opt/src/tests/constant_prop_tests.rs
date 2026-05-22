//! Tests for constant propagation optimization
use glyim_mir::*;
use glyim_type::TyCtx;
use glyim_test::{assert_mir, test_frozen_ty_ctx, with_fresh_ty_ctx};

/// Helper to build a simple MIR body with assignments and returns
fn build_test_body() -> (TyCtx, Body) {
    with_fresh_ty_ctx(|ctx_mut| {
        let mut body = Body::dummy(Default::default());
        let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(glyim_core::primitives::IntTy::I32));
        let local0 = LocalIdx::from_raw(0); // return place
        let local1 = LocalIdx::from_raw(1); // x
        let local2 = LocalIdx::from_raw(2); // y
        body.locals.push(LocalDecl { ty: i32_ty, mutability: glyim_core::primitives::Mutability::Mut, source_info: SourceInfo::new(glyim_span::Span::DUMMY) });
        body.locals.push(LocalDecl { ty: i32_ty, mutability: glyim_core::primitives::Mutability::Mut, source_info: SourceInfo::new(glyim_span::Span::DUMMY) });
        body.locals.push(LocalDecl { ty: i32_ty, mutability: glyim_core::primitives::Mutability::Mut, source_info: SourceInfo::new(glyim_span::Span::DUMMY) });
        body.basic_blocks = IndexVec::from_raw(vec![
            BasicBlockData {
                statements: vec![
                    Statement { kind: StatementKind::Assign(Place::new(local1), Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::Int(5), ty: i32_ty, span: glyim_span::Span::DUMMY }))), source_info: SourceInfo::new(glyim_span::Span::DUMMY) },
                    Statement { kind: StatementKind::Assign(Place::new(local2), Rvalue::BinaryOp(glyim_core::primitives::BinOp::Add, Box::new((Operand::Copy(Place::new(local1)), Operand::Constant(MirConst { kind: MirConstKind::Int(1), ty: i32_ty, span: glyim_span::Span::DUMMY }))))), source_info: SourceInfo::new(glyim_span::Span::DUMMY) },
                    Statement { kind: StatementKind::Assign(Place::new(local0), Operand::Move(Place::new(local2)).into()), source_info: SourceInfo::new(glyim_span::Span::DUMMY) },
                ],
                terminator: Terminator { kind: TerminatorKind::Return, source_info: SourceInfo::new(glyim_span::Span::DUMMY) },
                is_cleanup: false,
            }
        ]);
        (ctx_mut.freeze(), body)
    })
}

#[test]
fn constant_prop_single_block() {
    let (ctx, mut body) = build_test_body();
    super::constant_prop::run(&ctx, &mut body);
    // After propagation, the assignment to local2 should become Constant(6)
    let block = &body.basic_blocks[BasicBlockIdx::from_raw(0)];
    let stmt = &block.statements[1];
    match &stmt.kind {
        StatementKind::Assign(place, rvalue) => {
            assert_eq!(place.local, LocalIdx::from_raw(2));
            if let Rvalue::BinaryOp(_, box_ops) = rvalue {
                if let Operand::Constant(c) = &box_ops.0 {
                    assert_eq!(c.kind, MirConstKind::Int(6));
                } else {
                    panic!("Expected constant operand");
                }
            } else {
                panic!("Expected binary op");
            }
        }
        _ => panic!("Expected assign"),
    }
}

// TODO: add test for cross-block constant propagation
