use crate::*;
use glyim_core::arena::IndexVec;
use glyim_core::def_id::{CrateId, DefId, LocalDefId};
use glyim_core::primitives::Mutability;
use glyim_span::Span;
use glyim_type::Ty;

fn si() -> SourceInfo {
    SourceInfo::new(Span::DUMMY)
}

#[test]
fn body_with_many_locals() {
    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    for i in 1..=100u32 {
        body.locals.push(LocalDecl {
            ty: Ty::BOOL,
            mutability: if i % 2 == 0 { Mutability::Mut } else { Mutability::Not },
            source_info: si(),
        });
    }
    body.arg_count = 50;
    assert_eq!(body.locals.len(), 101);
    assert_eq!(body.args().len(), 50);
    for (i, arg) in body.args().iter().enumerate() {
        assert_eq!(arg.ty, Ty::BOOL);
        assert_eq!(arg.mutability, if (i + 1) % 2 == 0 { Mutability::Mut } else { Mutability::Not });
    }
}

#[test]
fn body_with_many_basic_blocks() {
    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));

    let num_blocks = 50;
    for i in 0..num_blocks {
        let next = if i + 1 < num_blocks {
            Some(BasicBlockIdx::from_raw((i + 1) as u32))
        } else {
            None
        };

        let term = if i == num_blocks - 1 {
            Terminator { kind: TerminatorKind::Return, source_info: si() }
        } else {
            Terminator {
                kind: TerminatorKind::Goto { target: next.unwrap() },
                source_info: si(),
            }
        };
        body.basic_blocks.push(BasicBlockData::new(term));
    }

    assert_eq!(body.basic_blocks.len(), 51);

    for i in 0..num_blocks - 1 {
        let bb_idx = BasicBlockIdx::from_raw((i + 1) as u32);
        if let TerminatorKind::Goto { target } = &body.basic_blocks[bb_idx].terminator.kind {
            assert_eq!(*target, BasicBlockIdx::from_raw((i + 2) as u32));
        }
    }
}

#[test]
fn body_with_deeply_nested_calls() {
    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    body.locals.push(LocalDecl { ty: Ty::UNIT, mutability: Mutability::Not, source_info: si() });

    let mut basic_block = BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: si(),
    });

    for i in 0..20 {
        basic_block.statements.push(Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(1)),
                Rvalue::Use(Operand::Constant(MirConst {
                    kind: MirConstKind::Uint(i as u128),
                    ty: Ty::UNIT,
                    span: Span::DUMMY,
                })),
            ),
            source_info: si(),
        });
    }

    body.basic_blocks.push(basic_block);
    assert_eq!(body.basic_blocks[BasicBlockIdx::from_raw(1)].statements.len(), 20);
}

#[test]
fn body_switch_with_many_branches() {
    let targets = SwitchTargets::new(
        Box::new((0..50u128).map(|i| (i, BasicBlockIdx::from_raw(i as u32 + 1))).collect::<Vec<_>>()),
        BasicBlockIdx::from_raw(51),
    );

    let term = Terminator {
        kind: TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::new(LocalIdx::from_raw(0))),
            switch_ty: Ty::BOOL,
            targets,
        },
        source_info: si(),
    };

    if let TerminatorKind::SwitchInt { targets, .. } = &term.kind {
        assert_eq!(targets.iter().count(), 50);
        assert_eq!(targets.otherwise(), BasicBlockIdx::from_raw(51));
    } else {
        panic!("Expected SwitchInt");
    }
}

#[test]
fn body_with_alternating_storage_live_dead() {
    let mut body = Body::dummy(DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)));
    for i in 1..=10u32 {
        body.locals.push(LocalDecl { ty: Ty::BOOL, mutability: Mutability::Not, source_info: si() });
    }

    let mut bb = BasicBlockData::new(Terminator { kind: TerminatorKind::Return, source_info: si() });

    for i in 1..=10u32 {
        bb.statements.push(Statement { kind: StatementKind::StorageLive(LocalIdx::from_raw(i)), source_info: si() });
        bb.statements.push(Statement {
            kind: StatementKind::Assign(
                Place::new(LocalIdx::from_raw(i)),
                Rvalue::Use(Operand::Constant(MirConst { kind: MirConstKind::Bool(true), ty: Ty::BOOL, span: Span::DUMMY })),
            ),
            source_info: si(),
        });
        bb.statements.push(Statement { kind: StatementKind::StorageDead(LocalIdx::from_raw(i)), source_info: si() });
    }

    body.basic_blocks.push(bb);
    assert_eq!(body.basic_blocks[BasicBlockIdx::from_raw(1)].statements.len(), 30);
}
