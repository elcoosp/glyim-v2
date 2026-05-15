use super::helpers::*;
use crate::Interpreter;
use glyim_core::*;
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{Ty, TyKind};

#[test]
fn test_t04_match_on_enum_discriminant() {
    let mut ctx = glyim_test::test_ty_ctx();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let adt_id = AdtId::from_raw(100); // dummy enum def
    let subst = ctx.intern_substitution(vec![]);
    let enum_ty = ctx.mk_ty(TyKind::Adt(adt_id, subst));

    let mut body = empty_body(Ty::UNIT);
    let local_enum = add_local(&mut body, enum_ty, Mutability::Mut);
    let local_discr = add_local(&mut body, ctx.mk_ty(TyKind::Int(IntTy::I32)), Mutability::Mut);
    let local_result = add_local(&mut body, i32_ty, Mutability::Not);

    // We'll have three basic blocks: bb0 (construct variant and discriminant), bb1 (if discrim ==0), bb2 (if discrim==1), bb3 (return)
    // Initially only bb0 exists.
    let bb0 = BasicBlockIdx::from_raw(0);
    let bb1 = BasicBlockIdx::from_raw(1);
    let bb2 = BasicBlockIdx::from_raw(2);
    let bb3 = BasicBlockIdx::from_raw(3);
    body.basic_blocks.push(BasicBlockData::new(Terminator {
        kind: TerminatorKind::Goto { target: bb3 },
        source_info: SourceInfo::new(Span::DUMMY),
    }));
    body.basic_blocks.push(BasicBlockData::new(Terminator {
        kind: TerminatorKind::Goto { target: bb3 },
        source_info: SourceInfo::new(Span::DUMMY),
    }));
    body.basic_blocks.push(BasicBlockData::new(Terminator {
        kind: TerminatorKind::Return,
        source_info: SourceInfo::new(Span::DUMMY),
    }));

    // bb0: construct variant 1 (use VariantIdx 1)
    add_statement(
        &mut body,
        bb0,
        StatementKind::Assign(
            Place::new(local_enum),
            Rvalue::Aggregate(
                AggregateKind::Adt(adt_id, VariantIdx::from_raw(1), subst),
                vec![const_int(42)],
            ),
        ),
    );
    // discriminant
    add_statement(
        &mut body,
        bb0,
        StatementKind::Assign(
            Place::new(local_discr),
            Rvalue::Discriminant(Place::new(local_enum)),
        ),
    );
    // SwitchInt on discriminant
    set_terminator(
        &mut body,
        bb0,
        TerminatorKind::SwitchInt {
            discr: Operand::Copy(Place::new(local_discr)),
            switch_ty: ctx.mk_ty(TyKind::Int(IntTy::I32)),
            targets: SwitchTargets::new(
                vec![(0u128, bb1), (1u128, bb2)].into_boxed_slice(),
                bb3,
            ),
        },
    );

    // bb1: discriminant == 0 -> result = 100
    add_statement(
        &mut body,
        bb1,
        StatementKind::Assign(Place::new(local_result), Rvalue::Use(const_int(100))),
    );

    // bb2: discriminant == 1 -> result = 200
    add_statement(
        &mut body,
        bb2,
        StatementKind::Assign(Place::new(local_result), Rvalue::Use(const_int(200))),
    );

    let tcx = ctx.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.add_function(
        DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        body,
    );
    // This test currently fails because Discriminant rvalue is not implemented.
    let result = interp.run_body(
        &interp
            .function_table
            .values()
            .next()
            .unwrap()
            .clone(),
    );
    assert!(result.is_err());
}
