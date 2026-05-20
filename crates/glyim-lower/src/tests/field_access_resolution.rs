use crate::lower::lower_body;
use crate::tests::mock_lower_ctx::TestLowerCtx;
use crate::tests::thir_builder::ThirBuilder;
use glyim_core::def_id::AdtId;
use glyim_core::primitives::IntTy;
use glyim_mir::ProjectionElem;
use glyim_test::test_ty_ctx;
use glyim_type::*;
use glyim_typeck::thir::{self, ExprKind};

/// S20-T01: Field access lowers to correct FieldIdx projection
#[test]
fn field_access_uses_resolved_field_idx() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let adt_id = AdtId::from_raw(42);
    let subst = ctx_mut.intern_substitution(vec![]);
    let struct_ty = ctx_mut.mk_adt(adt_id, subst);
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();

    let mut mock = TestLowerCtx::new(&ctx);
    // Register field "y" at index 1 for adt_id=42, variant=0
    let field_y = interner.intern("y");
    mock.add_field_index(adt_id, 0, field_y, FieldIdx::from_raw(1));

    let mut b = ThirBuilder::new(i32_ty, interner.clone());
    let mut stmts = Vec::new();
    b.add_let_binding("s", struct_ty, None, &mut stmts);

    let field_expr = b.expr(
        ExprKind::Field {
            receiver: Box::new(b.var_ref_expr("s", struct_ty)),
            field: field_y,
            ty: i32_ty,
        },
        i32_ty,
    );
    stmts.push(thir::Stmt::Expr { expr: field_expr });

    let body = b.into_body(stmts, vec![]);
    let result = lower_body(&mock, &body);

    // The MIR body should have a place with a Field(FieldIdx(1)) projection
    let found_field_proj = result.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            if let glyim_mir::StatementKind::Assign(_, rvalue) = &stmt.kind {
                if let glyim_mir::Rvalue::Use(operand) = rvalue {
                    if let glyim_mir::Operand::Copy(place) = operand {
                        return place.projection.iter().any(
                            |elem| matches!(elem, ProjectionElem::Field(idx) if idx.to_raw() == 1),
                        );
                    }
                }
            }
            false
        })
    });
    assert!(
        found_field_proj,
        "expected Field(FieldIdx(1)) projection in MIR"
    );
}

#[test]
fn field_access_with_no_resolution_emits_error_const() {
    let mut ctx_mut = test_ty_ctx();
    let i32_ty = ctx_mut.mk_ty(TyKind::Int(IntTy::I32));
    let adt_id = AdtId::from_raw(99);
    let subst = ctx_mut.intern_substitution(vec![]);
    let struct_ty = ctx_mut.mk_adt(adt_id, subst);
    let interner = ctx_mut.resolver().clone();
    let ctx = ctx_mut.freeze();

    // Mock with no field resolution for this ADT
    let mock = TestLowerCtx::new(&ctx);

    let mut b = ThirBuilder::new(i32_ty, interner.clone());
    let mut stmts = Vec::new();
    b.add_let_binding("s", struct_ty, None, &mut stmts);

    let field_z = interner.intern("z");
    let field_expr = b.expr(
        ExprKind::Field {
            receiver: Box::new(b.var_ref_expr("s", struct_ty)),
            field: field_z,
            ty: i32_ty,
        },
        i32_ty,
    );
    stmts.push(thir::Stmt::Expr { expr: field_expr });

    let body = b.into_body(stmts, vec![]);
    let result = lower_body(&mock, &body);

    // Should produce an Error constant since field resolution fails
    let found_error = result.body.basic_blocks.iter().any(|bb| {
        bb.statements.iter().any(|stmt| {
            if let glyim_mir::StatementKind::Assign(_, rvalue) = &stmt.kind {
                if let glyim_mir::Rvalue::Use(glyim_mir::Operand::Constant(c)) = rvalue {
                    return matches!(c.kind, glyim_mir::MirConstKind::Error);
                }
            }
            false
        })
    });
    assert!(
        found_error,
        "expected Error constant when field resolution fails"
    );
}
