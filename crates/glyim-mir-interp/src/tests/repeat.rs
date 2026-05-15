use super::helpers::*;
use crate::Interpreter;
use glyim_core::*;
use glyim_mir::*;
use glyim_span::Span;
use glyim_type::{Ty, TyKind};

#[test]
fn test_t09_repeat_rvalue() {
    // Repeat is currently a stub returning first element; test that it doesn't crash.
    let mut ctx = glyim_test::test_ty_ctx();
    let i32_ty = ctx.mk_ty(TyKind::Int(IntTy::I32));
    let array_ty = mk_array_ty(&mut ctx, i32_ty, 3);

    let mut body = empty_body(Ty::UNIT);
    let local_arr = add_local(&mut body, array_ty, Mutability::Mut);

    let bb0 = BasicBlockIdx::from_raw(0);
    add_statement(
        &mut body,
        bb0,
        StatementKind::Assign(
            Place::new(local_arr),
            Rvalue::Repeat(
                const_int(5),
                MirConst {
                    kind: MirConstKind::Int(3),
                    ty: i32_ty,
                    span: Span::DUMMY,
                },
            ),
        ),
    );

    let tcx = ctx.freeze();
    let mut interp = Interpreter::new(&tcx);
    interp.add_function(
        DefId::new(CrateId::from_raw(0), LocalDefId::from_raw(0)),
        body,
    );
    let result = interp.run_body(
        &interp
            .function_table
            .values()
            .next()
            .unwrap()
            .clone(),
    );
    // Repeat stub currently returns the operand value, so it shouldn't error.
    assert!(result.is_ok());
    let val = interp.get_local_value(local_arr).unwrap();
    assert_eq!(*val, crate::InterpValue::Int(5));
}
