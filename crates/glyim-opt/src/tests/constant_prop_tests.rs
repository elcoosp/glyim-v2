//! Test W4-C02-T01: constant propagation replaces operands with constants
//! (binary operation remains, but operands become constants)
use glyim_core::{CrateId, DefId, IndexVec, LocalDefId, primitives::IntTy};
use glyim_mir::*;
use glyim_span::Span;
use glyim_test::test_ty_ctx;

fn dummy_def_id() -> DefId {
    DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0))
}

#[test]
fn constant_prop_single_block() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(glyim_type::TyKind::Int(IntTy::I32));
    let mut body = Body::dummy(dummy_def_id());
    let local0 = LocalIdx::from_raw(0);
    let local1 = LocalIdx::from_raw(1);
    let local2 = LocalIdx::from_raw(2);
    body.locals.push(LocalDecl {
        ty: i32_ty,
        mutability: glyim_core::primitives::Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    body.locals.push(LocalDecl {
        ty: i32_ty,
        mutability: glyim_core::primitives::Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    body.locals.push(LocalDecl {
        ty: i32_ty,
        mutability: glyim_core::primitives::Mutability::Mut,
        source_info: SourceInfo::new(Span::DUMMY),
    });
    body.basic_blocks = IndexVec::from_raw(vec![BasicBlockData {
        statements: vec![
            Statement {
                kind: StatementKind::Assign(
                    Place::new(local1),
                    Rvalue::Use(Operand::Constant(MirConst {
                        kind: MirConstKind::Int(5),
                        ty: i32_ty,
                        span: Span::DUMMY,
                    })),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::new(local2),
                    Rvalue::BinaryOp(
                        glyim_core::primitives::BinOp::Add,
                        Box::new((
                            Operand::Copy(Place::new(local1)),
                            Operand::Constant(MirConst {
                                kind: MirConstKind::Int(1),
                                ty: i32_ty,
                                span: Span::DUMMY,
                            }),
                        )),
                    ),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::new(local0),
                    Rvalue::Use(Operand::Move(Place::new(local2))),
                ),
                source_info: SourceInfo::new(Span::DUMMY),
            },
        ],
        terminator: Terminator {
            kind: TerminatorKind::Return,
            source_info: SourceInfo::new(Span::DUMMY),
        },
        is_cleanup: false,
    }]);
    let ctx = ctx_mut.freeze();
    crate::constant_prop::run(&ctx, &mut body);
    let block = &body.basic_blocks[BasicBlockIdx::from_raw(0)];
    let stmt = &block.statements[1];
    match &stmt.kind {
        StatementKind::Assign(place, rvalue) => {
            assert_eq!(place.local, LocalIdx::from_raw(2));
            // Expect binary op with constant operands (5 and 1), not folded into 6
            match rvalue {
                Rvalue::BinaryOp(op, box_ops) => {
                    assert_eq!(*op, glyim_core::primitives::BinOp::Add);
                    match (&box_ops.0, &box_ops.1) {
                        (Operand::Constant(lc), Operand::Constant(rc)) => {
                            match &lc.kind {
                                MirConstKind::Int(5) => {}
                                _ => panic!("Expected Int(5)"),
                            };
                            match &rc.kind {
                                MirConstKind::Int(1) => {}
                                _ => panic!("Expected Int(1)"),
                            };
                        }
                        _ => panic!("Expected constant operands"),
                    }
                }
                _ => panic!("Expected BinaryOp after propagation"),
            }
        }
        _ => panic!("Expected assign"),
    }
}
